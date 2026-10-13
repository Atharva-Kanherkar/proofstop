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
    Task::load(&root().join("tasks/repository/naturalsize-rollover")).unwrap()
}
fn configs() -> (Config, Config) {
    (
        Config::load(&root().join("configs/gpu-nonthinking.json")).unwrap(),
        Config::load(&root().join("configs/gpu-thinking.json")).unwrap(),
    )
}
fn response(code: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"model":"fixture-model","choices":[{"finish_reason":"stop","message":{"content":format!("````python\n{code}````")}}],"usage":{"prompt_tokens":10,"completion_tokens":20}})).unwrap()
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
async fn actual_naturalsize_bug_and_fixed_target_use_the_same_runtime() {
    let task = task();
    let config = configs().0;
    for suite in ["acceptance", "evaluation"] {
        let bug = sandbox::verify_task(&task.source, &task, suite, &config)
            .await
            .unwrap();
        assert!(matches!(bug.verdict, Verdict::Failed), "{bug:?}");
        let fixed = sandbox::verify_task(
            include_str!("fixtures/repository/naturalsize-fixed.py"),
            &task,
            suite,
            &config,
        )
        .await
        .unwrap();
        assert!(matches!(fixed.verdict, Verdict::Passed), "{fixed:?}");
        let values: Vec<Value> = serde_json::from_str(bug.worker_stdout.as_ref().unwrap()).unwrap();
        assert_eq!(
            values[0]["result"],
            if suite == "acceptance" {
                "1000.0 kB"
            } else {
                "-1000.0 kB"
            }
        );
    }
}
#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn full_repository_file_with_nested_examples_can_be_repaired() {
    let task = task();
    let fixed = include_str!("fixtures/repository/naturalsize-fixed.py");
    let (endpoint, capture) = server(vec![response(&task.source), response(fixed)]).await;
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("repair");
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
    assert_eq!(result.outcome, "passed", "{result:?}");
    let requests = capture.await.unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests {
        for case in task.cases("evaluation").unwrap() {
            assert!(
                !request["messages"][0]["content"]
                    .as_str()
                    .unwrap()
                    .contains(&format!(
                        "\"name\": {}",
                        serde_json::to_string(&case.name).unwrap()
                    ))
            );
        }
    }
    assert_eq!(
        fs::read_to_string(output.join("candidate.py")).unwrap(),
        fixed
    );
}
