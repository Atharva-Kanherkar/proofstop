use proofstop::{
    online,
    sandbox::{self, Verdict},
    task::{Config, Task},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).into()
}
fn config() -> Config {
    Config::load(&root().join("configs/gpu-thinking.json")).unwrap()
}
fn task() -> Task {
    Task::load(&root().join("tasks/merge-ranges")).unwrap()
}

async fn server<F>(count: usize, mut reply: F) -> (String, tokio::task::JoinHandle<Vec<Value>>)
where
    F: FnMut(&str, &Value) -> Value + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let capture = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..count {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut buf = [0; 4096];
                let n = stream.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let header = String::from_utf8(bytes[..end].to_vec()).unwrap();
            let route = header.split_whitespace().nth(1).unwrap();
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|x| x.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < end + length {
                let mut buf = [0; 4096];
                let n = stream.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            let request: Value = serde_json::from_slice(&bytes[end..end + length]).unwrap();
            let mut response_value = reply(route, &request);
            let status = response_value
                .as_object_mut()
                .unwrap()
                .remove("_http_status")
                .and_then(|v| v.as_u64())
                .unwrap_or(200);
            let response = serde_json::to_vec(&response_value).unwrap();
            requests.push(json!({"route":route,"body":request}));
            let header = format!(
                "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&response).await.unwrap();
        }
        requests
    });
    (endpoint, capture)
}
fn tokenized(tokens: Value) -> Value {
    json!({"count":tokens.as_array().unwrap().len(),"tokens":tokens,"max_model_len":65536})
}
fn generated(prompt: &Value, token: u32, stop: bool, usage: bool) -> Value {
    let mut value = json!({"model":"m","choices":[{"finish_reason":if stop {"stop"} else {"length"},"prompt_token_ids":prompt,"token_ids":[token]}]});
    if usage {
        value["usage"] =
            json!({"prompt_tokens":prompt.as_array().unwrap().len(),"completion_tokens":1});
    }
    value
}

#[tokio::test]
async fn budget_exhaustion_and_unknown_usage_are_recorded() {
    let (endpoint, capture) = server(5, |route, body| match route {
        "/tokenize" => tokenized(json!([10, 11])),
        "/v1/completions" => generated(&body["prompt"], 1, false, false),
        "/detokenize" => json!({"prompt":"still reasoning"}),
        _ => panic!("unexpected route"),
    })
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config();
    config.max_tokens = 2;
    let options = online::Options {
        segment_tokens: 1,
        probe_tokens: 1,
        max_probes: 0,
    };
    let result = online::run(
        &task(),
        &config,
        &endpoint,
        "m",
        &tmp.path().join("run"),
        &options,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "failed");
    assert!(result.prompt_tokens.is_none());
    assert!(result.completion_tokens.is_none());
    let requests = capture.await.unwrap();
    assert_eq!(requests[3]["body"]["prompt"], json!([10, 11, 1]));
    assert!(
        online::run(
            &task(),
            &config,
            &endpoint,
            "m",
            &tmp.path().join("run"),
            &options,
            "harness_validation"
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn invalid_echo_aborts_before_probe_and_preserves_wire() {
    let (endpoint, capture) = server(3, |route, body| match route {
        "/tokenize" => {
            if body.get("messages").is_some() {
                tokenized(json!([10, 11]))
            } else {
                tokenized(json!([99]))
            }
        }
        "/v1/completions" => generated(&json!([999]), 1, false, true),
        _ => panic!("unexpected request"),
    })
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let config = config();
    let options = online::Options {
        segment_tokens: 1,
        probe_tokens: 1,
        max_probes: 1,
    };
    let result = online::run(
        &task(),
        &config,
        &endpoint,
        "m",
        &tmp.path().join("run"),
        &options,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "failed");
    assert!(result.error.unwrap().contains("exact prompt"));
    assert_eq!(capture.await.unwrap().len(), 3);
    assert!(tmp.path().join("run/003-main.response.json").exists());
}

async fn probe_scenario(first_bad: bool, overfit: bool, inconclusive: bool) {
    let good = if inconclusive {
        "while True: pass\n".to_string()
    } else {
        fs::read_to_string(root().join(if overfit {
            "tests/fixtures/overfit.py"
        } else {
            "tests/fixtures/reference.py"
        }))
        .unwrap()
    };
    let bad = task().source;
    let count = if first_bad { 10 } else { 6 };
    let mut mains = 0;
    let mut probes = 0;
    let (endpoint, capture) = server(count, move |route, body| match route {
        "/tokenize" => {
            if body.get("messages").is_some() {
                tokenized(json!([10, 11]))
            } else {
                tokenized(json!([99]))
            }
        }
        "/v1/completions" => {
            if body["prompt"].as_array().unwrap().last() == Some(&json!(99)) {
                probes += 1;
                generated(
                    &body["prompt"],
                    if first_bad && probes == 1 { 2 } else { 4 },
                    true,
                    true,
                )
            } else {
                mains += 1;
                generated(&body["prompt"], if mains == 1 { 1 } else { 3 }, false, true)
            }
        }
        "/detokenize" => {
            let tokens = body["tokens"].as_array().unwrap();
            if tokens.last() == Some(&json!(2)) {
                json!({"prompt":format!("reasoning\n</think>\n```python\n{bad}```")})
            } else if tokens.last() == Some(&json!(4)) {
                json!({"prompt":format!("reasoning\n</think>\n```python\n{good}```")})
            } else {
                json!({"prompt":"still reasoning"})
            }
        }
        _ => panic!("unexpected request"),
    })
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config();
    config.max_tokens = 4;
    if inconclusive {
        config.verifier_timeout_seconds = 1;
    }
    let options = online::Options {
        segment_tokens: 1,
        probe_tokens: 1,
        max_probes: 2,
    };
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        online::run(
            &task(),
            &config,
            &endpoint,
            "m",
            &tmp.path().join("run"),
            &options,
            "harness_validation",
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        result.outcome,
        if inconclusive {
            "inconclusive"
        } else if overfit {
            "failed_checks"
        } else {
            "passed"
        }
    );
    let requests = capture.await.unwrap();
    let generations: Vec<_> = requests
        .iter()
        .filter(|r| r["route"] == "/v1/completions")
        .collect();
    assert_eq!(generations.len(), if first_bad { 4 } else { 2 });
    if first_bad {
        assert_eq!(generations[2]["body"]["prompt"], json!([10, 11, 1]));
    }
    assert_eq!(result.completion_tokens, Some(generations.len() as u64));
    let events = fs::read_to_string(tmp.path().join("run/events.jsonl")).unwrap();
    let parsed: Vec<Value> = events
        .lines()
        .map(|x| serde_json::from_str(x).unwrap())
        .collect();
    let stop = parsed
        .iter()
        .rposition(|e| {
            e["event"]
                == if inconclusive {
                    "acceptance_finished"
                } else {
                    "generation_stopped_on_acceptance"
                }
        })
        .unwrap();
    assert!(
        !parsed[stop + 1..]
            .iter()
            .any(|e| e["event"] == "request_started")
    );
    if inconclusive {
        assert!(result.evaluation.is_none());
    } else {
        assert_eq!(
            result.evaluation.unwrap()["verdict"],
            if overfit { "failed" } else { "passed" }
        );
    }
}
#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn first_passing_probe_stops_generation() {
    probe_scenario(false, false, false).await;
}
#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn failed_probe_cannot_contaminate_continuation() {
    probe_scenario(true, false, false).await;
}
#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn evaluation_failure_does_not_resume_generation() {
    probe_scenario(false, true, false).await;
}
#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn reference_passes_multi_bug_fixture() {
    let task = Task::load(&root().join("tasks/merge-ranges-multi-bug")).unwrap();
    let config = config();
    assert!(matches!(
        sandbox::verify(&task.source, task.cases("acceptance").unwrap(), &config)
            .await
            .unwrap()
            .verdict,
        Verdict::Failed
    ));
    let reference = fs::read_to_string(root().join("tests/fixtures/reference.py")).unwrap();
    for suite in ["acceptance", "evaluation"] {
        assert!(matches!(
            sandbox::verify(&reference, task.cases(suite).unwrap(), &config)
                .await
                .unwrap()
                .verdict,
            Verdict::Passed
        ));
    }
}

#[tokio::test]
async fn failed_generation_http_keeps_attempt_and_unknown_usage() {
    let (endpoint, capture) = server(2, |route, _| match route {
        "/tokenize" => tokenized(json!([10, 11])),
        "/v1/completions" => json!({"_http_status":503,"error":"synthetic failure"}),
        _ => panic!("unexpected request"),
    })
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let options = online::Options {
        segment_tokens: 1,
        probe_tokens: 1,
        max_probes: 0,
    };
    let result = online::run(
        &task(),
        &config(),
        &endpoint,
        "m",
        &tmp.path().join("run"),
        &options,
        "harness_validation",
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "failed");
    assert!(result.error.unwrap().contains("HTTP status 503"));
    assert!(result.prompt_tokens.is_none());
    assert!(result.completion_tokens.is_none());
    assert!(tmp.path().join("run/002-main.request.json").exists());
    assert!(tmp.path().join("run/002-main.response.json").exists());
    assert_eq!(capture.await.unwrap().len(), 2);
}

#[tokio::test]
#[ignore = "requires real Docker and pinned Python image"]
async fn inconclusive_verifier_aborts_without_accepting_or_resuming() {
    probe_scenario(false, false, true).await;
}
