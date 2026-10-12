use proofstop::{
    repair,
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
fn configs() -> (Config, Config) {
    (
        Config::load(&root().join("configs/gpu-nonthinking.json")).unwrap(),
        Config::load(&root().join("configs/gpu-thinking.json")).unwrap(),
    )
}
async fn single_server(status: u16, response: Vec<u8>) -> (String, tokio::task::JoinHandle<Value>) {
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
async fn http_failure_and_truncation_do_not_retry_and_refuse_overwrite() {
    for (status,body,tokens) in [(503,br#"{"error":"unavailable"}"#.to_vec(),None),(200,serde_json::to_vec(&json!({"model":"fixture-model","choices":[{"finish_reason":"length","message":{"content":"```python\nx=1"}}],"usage":{"prompt_tokens":10,"completion_tokens":4096}})).unwrap(),Some(4096))] {
        let (endpoint,capture)=single_server(status,body.clone()).await;
        let tmp=tempfile::tempdir().unwrap();let output=tmp.path().join("run");let (direct,thinking)=configs();
        let result=repair::run(&task(),&direct,&thinking,&endpoint,"fixture-model",&output,"harness_validation").await.unwrap();
        assert_eq!(result.outcome,"failed");assert_eq!(result.completion_tokens,tokens);
        assert_eq!(fs::read(output.join("direct-response.json")).unwrap(),body);
        assert!(!output.join("repair-request.json").exists());capture.await.unwrap();
        assert!(repair::run(&task(),&direct,&thinking,&endpoint,"fixture-model",&output,"harness_validation").await.is_err());
    }
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn direct_pass_and_evaluation_failure_never_repair() {
    for (code, outcome) in [
        (include_str!("fixtures/reference.py"), "passed"),
        (include_str!("fixtures/overfit.py"), "failed_checks"),
    ] {
        let (endpoint, capture) = server(vec![response(code)]).await;
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("run");
        let (direct, thinking) = configs();
        let result = repair::run(
            &task(),
            &direct,
            &thinking,
            &endpoint,
            "fixture-model",
            &output,
            "harness_validation",
        )
        .await
        .unwrap();
        assert_eq!(result.outcome, outcome, "{result:?}");
        assert_eq!(result.completion_tokens, Some(20));
        assert!(result.evaluation.is_some());
        assert!(!output.join("repair-request.json").exists());
        let requests = capture.await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["chat_template_kwargs"]["enable_thinking"],
            false
        );
    }
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn visible_failure_gets_exactly_one_repair_success_or_failure() {
    for (second, outcome) in [
        (include_str!("fixtures/reference.py"), "passed"),
        ("def merge_ranges(value):\n    return []\n", "failed_checks"),
    ] {
        let task = task();
        let (endpoint, capture) = server(vec![response(&task.source), response(second)]).await;
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
        assert_eq!(result.outcome, outcome, "{result:?}");
        assert_eq!(result.completion_tokens, Some(40));
        assert_eq!(result.prompt_tokens, Some(20));
        let requests = capture.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0]["chat_template_kwargs"]["enable_thinking"],
            false
        );
        assert_eq!(requests[1]["chat_template_kwargs"]["enable_thinking"], true);
        let prompt = requests[1]["messages"][0]["content"].as_str().unwrap();
        assert!(prompt.contains("Visible acceptance results"));
        assert!(prompt.contains("\"passed\": false"));
        for case in task.cases("evaluation").unwrap() {
            assert!(!prompt.contains(&case.name));
        }
        assert_eq!(
            fs::read_to_string(output.join("candidate.py")).unwrap(),
            second
        );
    }
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn inconclusive_verifier_aborts_without_repair_or_evaluation() {
    let (endpoint, capture) = server(vec![response("while True: pass\n")]).await;
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("run");
    let (mut direct, mut thinking) = configs();
    direct.verifier_timeout_seconds = 2;
    thinking.verifier_timeout_seconds = 2;
    let result = repair::run(
        &task(),
        &direct,
        &thinking,
        &endpoint,
        "fixture-model",
        &output,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "inconclusive");
    assert!(result.evaluation.is_none());
    assert!(!output.join("repair-request.json").exists());
    capture.await.unwrap();
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn all_ten_pinned_bugs_fail_and_references_pass() {
    let set: Value =
        serde_json::from_slice(&fs::read(root().join("tasks/dev/set.json")).unwrap()).unwrap();
    assert_eq!(set["tasks"].as_array().unwrap().len(), 10);
    let config = configs().0;
    for entry in set["tasks"].as_array().unwrap() {
        let path = root().join(entry["path"].as_str().unwrap());
        assert_eq!(
            proofstop::task::hash(&fs::read(path.join("manifest.json")).unwrap()),
            entry["manifest_sha256"].as_str().unwrap()
        );
        let task = Task::load(&path).unwrap();
        let bug = sandbox::verify_function(
            &task.source,
            task.cases("acceptance").unwrap(),
            &config,
            task.entrypoint(),
        )
        .await
        .unwrap();
        assert!(
            matches!(bug.verdict, Verdict::Failed),
            "{} {bug:?}",
            task.manifest.id
        );
        let reference =
            fs::read_to_string(root().join(format!("tests/fixtures/dev/{}.py", task.manifest.id)))
                .unwrap();
        for suite in ["acceptance", "evaluation"] {
            let result = sandbox::verify_function(
                &reference,
                task.cases(suite).unwrap(),
                &config,
                task.entrypoint(),
            )
            .await
            .unwrap();
            assert!(
                matches!(result.verdict, Verdict::Passed),
                "{} {suite} {result:?}",
                task.manifest.id
            );
        }
    }
}
