use proofstop::{
    repair, run,
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
    Task::load(&root().join("tasks/repository/h11-chunk-footer")).unwrap()
}
fn configs() -> (Config, Config) {
    (
        Config::load(&root().join("configs/gpu-nonthinking.json")).unwrap(),
        Config::load(&root().join("configs/gpu-thinking.json")).unwrap(),
    )
}
fn response(code: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"model":"fixture-model","choices":[{"finish_reason":"stop","message":{"content":format!("```python\n{code}```")}}],"usage":{"prompt_tokens":10,"completion_tokens":20}})).unwrap()
}
async fn server(responses: Vec<Vec<u8>>) -> (String, tokio::task::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let mut captured = Vec::new();
        for response in responses {
            let (mut socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(20), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut buf = [0u8; 2048];
                let n = socket.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..end]);
            assert!(headers.starts_with("POST /v1/chat/completions "));
            let size: usize = headers
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < end + size {
                let mut buf = [0u8; 2048];
                let n = socket.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            captured.push(serde_json::from_slice(&bytes[end..end + size]).unwrap());
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        response.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(&response).await.unwrap();
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(700), listener.accept())
                .await
                .is_err(),
            "unexpected extra request"
        );
        captured
    });
    (endpoint, handle)
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn historical_bug_and_upstream_control_exercise_real_connection() {
    let task = task();
    let config = configs().0;
    for suite in ["acceptance", "evaluation"] {
        let bug = sandbox::verify_task(&task.source, &task, suite, &config)
            .await
            .unwrap();
        assert!(matches!(bug.verdict, Verdict::Failed), "{bug:?}");
        assert_eq!(bug.passed, if suite == "acceptance" { 3 } else { 9 });
        let observations: Vec<Value> =
            serde_json::from_str(bug.worker_stdout.as_ref().unwrap()).unwrap();
        let malformed = if suite == "acceptance" { 1 } else { 0 };
        assert_eq!(observations[malformed]["result"]["complete"], true);
        assert!(observations[malformed].get("error").is_none());
        let fixed = sandbox::verify_task(
            include_str!("fixtures/repository/h11-readers-fixed.py"),
            &task,
            suite,
            &config,
        )
        .await
        .unwrap();
        assert!(matches!(fixed.verdict, Verdict::Passed), "{fixed:?}");
        let observations: Vec<Value> =
            serde_json::from_str(fixed.worker_stdout.as_ref().unwrap()).unwrap();
        assert_eq!(observations[malformed]["error"], "RemoteProtocolError");
    }
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn incomplete_fix_passes_acceptance_but_evaluation_cannot_resume_generation() {
    let task = task();
    let incomplete=task.source.replace("            self._bytes_to_discard -= len(data)","            if self._bytes_to_discard == 2 and data[:1] != b'\\r':\n                raise LocalProtocolError('bad footer')\n            self._bytes_to_discard -= len(data)");
    let config = configs().0;
    let acceptance = sandbox::verify_task(&incomplete, &task, "acceptance", &config)
        .await
        .unwrap();
    assert!(
        matches!(acceptance.verdict, Verdict::Passed),
        "{acceptance:?}"
    );
    let evaluation = sandbox::verify_task(&incomplete, &task, "evaluation", &config)
        .await
        .unwrap();
    assert!(
        matches!(evaluation.verdict, Verdict::Failed),
        "{evaluation:?}"
    );
    let (endpoint, capture) = server(vec![response(&incomplete)]).await;
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("run");
    let (direct, thinking) = configs();
    let result = repair::run(
        &task,
        &direct,
        &thinking,
        &endpoint,
        "fixture-model",
        &output,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "failed_checks");
    assert!(!output.join("repair-request.json").exists());
    assert_eq!(capture.await.unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn repository_baseline_and_feedback_repair_run_end_to_end() {
    let fixed = include_str!("fixtures/repository/h11-readers-fixed.py");
    let task = task();
    let (direct, thinking) = configs();
    let (endpoint, capture) = server(vec![response(fixed)]).await;
    let tmp = tempfile::tempdir().unwrap();
    let result = run::baseline(
        &task,
        &direct,
        &endpoint,
        "fixture-model",
        &tmp.path().join("baseline"),
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "passed", "{result:?}");
    assert_eq!(capture.await.unwrap().len(), 1);
    let (endpoint, capture) = server(vec![response(&task.source), response(fixed)]).await;
    let output = tmp.path().join("repair");
    let result = repair::run(
        &task,
        &direct,
        &thinking,
        &endpoint,
        "fixture-model",
        &output,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "passed", "{result:?}");
    let requests = capture.await.unwrap();
    assert_eq!(requests.len(), 2);
    let prompt = requests[1]["messages"][0]["content"].as_str().unwrap();
    assert!(prompt.contains("Previous h11/_readers.py"));
    for case in task.cases("evaluation").unwrap() {
        assert!(!prompt.contains(&case.name));
    }
    assert_eq!(
        fs::read_to_string(output.join("candidate.py")).unwrap(),
        fixed
    );
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn repository_mount_contains_only_runtime_and_worker() {
    let code = format!(
        "{}\n{}",
        include_str!("fixtures/repository/h11-readers-fixed.py"),
        r#"
import os
assert os.getuid() == 65534
assert set(os.listdir('/work')) == {'repo', 'worker.py'}
assert set(os.listdir('/work/repo')) == {'h11', 'LICENSE.txt'}
assert set(os.listdir('/work/repo/h11')) == {'__init__.py', '_abnf.py', '_connection.py', '_events.py', '_headers.py', '_readers.py', '_receivebuffer.py', '_state.py', '_util.py', '_version.py', '_writers.py', 'py.typed'}
try:
    open('/work/repo/h11/_connection.py','w')
except OSError:
    pass
else:
    raise RuntimeError('dependency is writable')
"#
    );
    let result = sandbox::verify_task(&code, &task(), "evaluation", &configs().0)
        .await
        .unwrap();
    assert!(matches!(result.verdict, Verdict::Passed), "{result:?}");
}
