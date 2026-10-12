use proofstop::{
    run,
    sandbox::{self, Verdict},
    task::{Config, Task},
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn task() -> Task {
    Task::load(&root().join("tasks/merge-ranges")).unwrap()
}
fn config() -> Config {
    Config::load(&root().join("configs/local.json")).unwrap()
}

async fn capture_server(
    status: u16,
    response: Vec<u8>,
) -> (String, tokio::task::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let capture = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let header_end;
        loop {
            let mut buffer = [0u8; 1024];
            let n = socket.read(&mut buffer).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = index + 4;
                break;
            }
        }
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        assert!(headers.starts_with("POST /v1/chat/completions "));
        let size: usize = headers
            .lines()
            .find_map(|line| {
                line.to_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse().unwrap())
            })
            .unwrap();
        while bytes.len() < header_end + size {
            let mut buffer = [0u8; 1024];
            let n = socket.read(&mut buffer).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
        }
        let request: Value = serde_json::from_slice(&bytes[header_end..header_end + size]).unwrap();
        let head = format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.len()
        );
        socket.write_all(head.as_bytes()).await.unwrap();
        socket.write_all(&response).await.unwrap();
        request
    });
    (endpoint, capture)
}

#[tokio::test]
async fn failed_http_request_is_captured_and_cannot_overwrite_a_run() {
    let body = br#"{"error":"synthetic server failure"}"#.to_vec();
    let (endpoint, capture) = capture_server(503, body.clone()).await;
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("run");
    let mut sampling = config();
    sampling.top_p = Some(0.95);
    sampling.top_k = Some(20);
    let result = run::baseline(
        &task(),
        &sampling,
        &endpoint,
        "fixture-model",
        &output,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "failed");
    assert!(result.allocated_gpu_seconds.is_none());
    assert_eq!(fs::read(output.join("response.json")).unwrap(), body);
    let request = capture.await.unwrap();
    assert_eq!(request["stream"], false);
    assert_eq!(request["model"], "fixture-model");
    assert_eq!(request["top_p"], 0.95);
    assert_eq!(request["top_k"], 20);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(output.join("request.json")).unwrap()).unwrap(),
        request
    );
    assert!(!request.to_string().contains("negative_nested"));
    assert!(
        fs::read_to_string(output.join("events.jsonl"))
            .unwrap()
            .contains("run_finished")
    );
    let original = fs::read(output.join("summary.json")).unwrap();
    assert!(
        run::baseline(
            &task(),
            &config(),
            &endpoint,
            "fixture-model",
            &output,
            "harness_validation"
        )
        .await
        .is_err()
    );
    assert_eq!(fs::read(output.join("summary.json")).unwrap(), original);
}

#[tokio::test]
async fn truncated_completion_is_a_recorded_failure() {
    let body = serde_json::to_vec(&json!({"model":"fixture-model","choices":[{"finish_reason":"length","message":{"content":"```python\nx=1"}}],"usage":{"prompt_tokens":100,"completion_tokens":4096}})).unwrap();
    let (endpoint, capture) = capture_server(200, body).await;
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("run");
    let result = run::baseline(
        &task(),
        &config(),
        &endpoint,
        "fixture-model",
        &output,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "failed");
    assert_eq!(result.completion_tokens, Some(4096));
    assert!(!output.join("candidate.py").exists());
    capture.await.unwrap();
}

#[tokio::test]
#[ignore = "requires real Docker and the pinned Python image"]
async fn real_docker_distinguishes_bug_reference_and_overfit() {
    let task = task();
    let config = config();
    let bug = sandbox::verify(&task.source, task.cases("acceptance").unwrap(), &config)
        .await
        .unwrap();
    assert!(matches!(bug.verdict, Verdict::Failed));
    assert_eq!(bug.passed, 3);
    let reference = include_str!("fixtures/reference.py");
    for suite in ["acceptance", "evaluation"] {
        let result = sandbox::verify(reference, task.cases(suite).unwrap(), &config)
            .await
            .unwrap();
        assert!(matches!(result.verdict, Verdict::Passed), "{result:?}");
    }
    let overfit = include_str!("fixtures/overfit.py");
    let accept = sandbox::verify(overfit, task.cases("acceptance").unwrap(), &config)
        .await
        .unwrap();
    let evaluate = sandbox::verify(overfit, task.cases("evaluation").unwrap(), &config)
        .await
        .unwrap();
    assert!(matches!(accept.verdict, Verdict::Passed));
    assert!(matches!(evaluate.verdict, Verdict::Failed));
}

#[tokio::test]
#[ignore = "requires real Docker and the pinned Python image"]
async fn candidate_faults_are_failures_not_harness_errors() {
    let task = task();
    let config = config();
    for code in [
        "def merge_ranges(ranges)\n    return []\n",
        "def merge_ranges(ranges):\n    return {1}\n",
        "def merge_ranges(ranges):\n    raise SystemExit(0)\n",
        "def merge_ranges(ranges):\n    return float('nan')\n",
        "raise ValueError('broken at import')\n",
    ] {
        let result = sandbox::verify(code, task.cases("evaluation").unwrap(), &config)
            .await
            .unwrap();
        assert!(
            matches!(result.verdict, Verdict::Failed),
            "{code:?} {result:?}"
        );
        assert_eq!(result.passed, 0, "{code:?} {result:?}");
    }
}

#[tokio::test]
#[ignore = "requires real Docker and the pinned Python image"]
async fn verifier_timeout_and_noise_fail_closed_and_remove_containers() {
    let task = task();
    let mut config = config();
    config.verifier_timeout_seconds = 2;
    let result = sandbox::verify(
        "while True: pass\n",
        task.cases("acceptance").unwrap(),
        &config,
    )
    .await
    .unwrap();
    assert!(matches!(result.verdict, Verdict::Inconclusive));
    assert!(result.reason.contains("timed out"), "{result:?}");
    let inspect = std::process::Command::new("docker")
        .args(["inspect", &result.container_name])
        .output()
        .unwrap();
    assert!(!inspect.status.success());
    let result = sandbox::verify(
        "import os\nos.write(1, b'not json')\nos._exit(0)\n",
        task.cases("acceptance").unwrap(),
        &config,
    )
    .await
    .unwrap();
    assert!(matches!(result.verdict, Verdict::Inconclusive));
}

#[tokio::test]
#[ignore = "requires real Docker and the pinned Python image"]
async fn mock_inference_with_real_docker_exercises_complete_baseline() {
    let source = include_str!("fixtures/reference.py");
    let body = serde_json::to_vec(&json!({"model":"fixture-model","choices":[{"finish_reason":"stop","message":{"content":format!("```python\n{source}```")}}]})).unwrap();
    let (endpoint, capture) = capture_server(200, body.clone()).await;
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("run");
    let result = run::baseline(
        &task(),
        &config(),
        &endpoint,
        "fixture-model",
        &output,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "passed", "{result:?}");
    assert_eq!(result.evidence_class, "harness_validation");
    assert!(result.prompt_tokens.is_none());
    assert!(result.allocated_gpu_seconds.is_none());
    assert_eq!(fs::read(output.join("response.json")).unwrap(), body);
    assert_eq!(
        fs::read_to_string(output.join("candidate.py")).unwrap(),
        source
    );
    let captured = capture.await.unwrap();
    assert_eq!(
        captured,
        serde_json::from_slice::<Value>(&fs::read(output.join("request.json")).unwrap()).unwrap()
    );
}

#[tokio::test]
#[ignore = "requires real Docker and the pinned Python image"]
async fn candidate_cannot_reach_network_or_write_source_and_sees_no_answers() {
    let task = task();
    let code = format!(
        "{}\n{}",
        include_str!("fixtures/reference.py"),
        r#"
import os
import socket
assert os.getuid() == 65534
assert set(os.listdir('/work')) == {'ranges.py', 'worker.py'}
try:
    open('/work/injected.py', 'w')
except OSError:
    pass
else:
    raise RuntimeError('source mount is writable')
with socket.socket() as connection:
    connection.settimeout(1)
    try:
        connection.connect(('1.1.1.1', 53))
    except OSError:
        pass
    else:
        raise RuntimeError('network is reachable')
"#
    );
    let result = sandbox::verify(&code, task.cases("evaluation").unwrap(), &config())
        .await
        .unwrap();
    assert!(matches!(result.verdict, Verdict::Passed), "{result:?}");
}
