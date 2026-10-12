use crate::task::{Case, Config};
use anyhow::{Result, ensure};
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};
use uuid::Uuid;

const OUTPUT_LIMIT: u64 = 64 * 1024;
const WORKER: &str = include_str!("worker.py");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Passed,
    Failed,
    Inconclusive,
}

#[derive(Debug, Serialize)]
pub struct Verification {
    pub verdict: Verdict,
    pub cases: usize,
    pub passed: usize,
    pub checks: Vec<bool>,
    pub wall_ms: u128,
    pub reason: String,
    pub container_name: String,
}

pub fn validate_image(image: &str) -> Result<()> {
    let Some((repository, digest)) = image.split_once("@sha256:") else {
        anyhow::bail!("verifier image must be pinned as repository@sha256:digest");
    };
    ensure!(
        !repository.is_empty()
            && !repository.starts_with('-')
            && repository
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"./:_-".contains(&c)),
        "invalid image repository"
    );
    ensure!(
        digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid image digest"
    );
    Ok(())
}

pub fn docker_args(image: &str, mount: &str, name: &str) -> Vec<String> {
    [
        "run",
        "--rm",
        "--pull=never",
        "--name",
        name,
        "--label",
        "proofstop.verifier=true",
        "--network=none",
        "--read-only",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--user=65534:65534",
        "--pids-limit=32",
        "--cpus=0.5",
        "--memory=128m",
        "--memory-swap=128m",
        "--ulimit=nofile=64:64",
        "--init",
        "--tmpfs=/tmp:rw,noexec,nosuid,size=16m",
        "--mount",
        mount,
        "--workdir=/work",
        "-i",
        image,
        "python",
        "-I",
        "-B",
        "/work/worker.py",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

async fn bounded_read(reader: impl AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(OUTPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .await?;
    Ok(bytes)
}

/// All expected answers stay in Rust. Only inputs and candidate code enter Docker.
pub async fn verify(code: &str, cases: &[Case], config: &Config) -> Result<Verification> {
    verify_function(code, cases, config, "merge_ranges").await
}

pub async fn verify_function(
    code: &str,
    cases: &[Case],
    config: &Config,
    entrypoint: &str,
) -> Result<Verification> {
    ensure!(
        ["merge_ranges", "solve"].contains(&entrypoint),
        "unsupported entrypoint"
    );
    validate_image(&config.image)?;
    ensure!(!cases.is_empty(), "cannot verify an empty suite");
    ensure!(code.len() <= 64 * 1024, "candidate exceeds 64 KiB");
    let started = Instant::now();
    let name = format!("proofstop-{}", Uuid::new_v4());
    let workspace = tempfile::tempdir()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(workspace.path(), fs::Permissions::from_mode(0o755))?;
    }
    fs::write(workspace.path().join("ranges.py"), code)?;
    fs::write(workspace.path().join("worker.py"), WORKER)?;
    let workspace_path = workspace
        .path()
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 sandbox path"))?;
    ensure!(
        !workspace_path.contains(','),
        "unsupported comma in sandbox path"
    );
    let mount = format!("type=bind,src={workspace_path},dst=/work,readonly");
    let mut child = Command::new("docker")
        .args(docker_args(&config.image, &mount, &name))
        .arg(entrypoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = tokio::spawn(bounded_read(child.stdout.take().unwrap()));
    let stderr = tokio::spawn(bounded_read(child.stderr.take().unwrap()));
    let inputs: Vec<_> = cases.iter().map(|c| &c.input).collect();
    let payload = serde_json::to_vec(&inputs)?;
    let mut stdin = child.stdin.take().unwrap();
    let write = tokio::spawn(async move {
        stdin.write_all(&payload).await?;
        stdin.shutdown().await
    });
    let outcome = timeout(
        Duration::from_secs(config.verifier_timeout_seconds),
        child.wait(),
    )
    .await;
    let timed_out = outcome.is_err();
    // Removing this uniquely named container kills all its processes, not just
    // the attached Docker client. Cleanup is mandatory even after client errors.
    let cleanup = timeout(
        Duration::from_secs(10),
        Command::new("docker")
            .args(["rm", "--force", &name])
            .output(),
    )
    .await;
    let cleanup_ok = match cleanup {
        Ok(Ok(output)) => {
            output.status.success()
                || String::from_utf8_lossy(&output.stderr).contains("No such container")
        }
        _ => false,
    };
    if timed_out {
        let _ = child.kill().await;
    }
    // On infrastructure failures do not keep a blocked pipe writer alive.
    write.abort();
    let out = timeout(Duration::from_secs(5), stdout).await;
    let err = timeout(Duration::from_secs(5), stderr).await;
    let mut result = Verification {
        verdict: Verdict::Inconclusive,
        cases: cases.len(),
        passed: 0,
        checks: Vec::new(),
        wall_ms: started.elapsed().as_millis(),
        reason: String::new(),
        container_name: name,
    };
    if !cleanup_ok {
        result.reason = "container cleanup failed".into();
        return Ok(result);
    }
    if timed_out {
        result.reason = "verifier timed out; container removed".into();
        return Ok(result);
    }
    match outcome {
        Ok(Ok(status)) if status.success() => {}
        _ => {
            result.reason = "verifier process or Docker failed".into();
            return Ok(result);
        }
    }
    let (Ok(Ok(Ok(output))), Ok(Ok(Ok(errors)))) = (out, err) else {
        result.reason = "could not capture bounded verifier output".into();
        return Ok(result);
    };
    if output.len() as u64 > OUTPUT_LIMIT || errors.len() as u64 > OUTPUT_LIMIT {
        result.reason = "verifier output exceeded limit".into();
        return Ok(result);
    }
    match score_checks(&output, cases) {
        Ok(checks) => {
            let passed = checks.iter().filter(|p| **p).count();
            result.checks = checks;
            result.passed = passed;
            result.verdict = if passed == cases.len() {
                Verdict::Passed
            } else {
                Verdict::Failed
            };
            result.reason = format!("{passed} of {} checks passed", cases.len());
        }
        Err(_) => result.reason = "invalid verifier output".into(),
    }
    Ok(result)
}

#[cfg(test)]
fn score_output(output: &[u8], cases: &[Case]) -> Result<usize> {
    Ok(score_checks(output, cases)?.iter().filter(|p| **p).count())
}

fn score_checks(output: &[u8], cases: &[Case]) -> Result<Vec<bool>> {
    let records: Vec<Value> = serde_json::from_slice(output)?;
    ensure!(records.len() == cases.len(), "wrong verifier result count");
    let mut checks = Vec::new();
    for (record, case) in records.iter().zip(cases) {
        ensure!(
            record.is_object() && record.get("input_after").is_some(),
            "malformed verifier record"
        );
        let matches = match (&case.expected, &case.error) {
            (Some(expected), None) => {
                record
                    .get("result")
                    .is_some_and(|actual| json_equal(actual, expected))
                    && record.get("error").is_none()
            }
            (None, Some(error)) => {
                record.get("error").and_then(Value::as_str) == Some(error)
                    && record.get("result").is_none()
            }
            _ => false,
        };
        checks.push(matches && record["input_after"] == case.input);
    }
    Ok(checks)
}

// JSON distinguishes 4 from 4.0 internally; compare numeric values without rounding
// large integer answers through f64.
fn json_equal(actual: &Value, expected: &Value) -> bool {
    if actual == expected {
        return true;
    }
    match (actual, expected) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| json_equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| json_equal(value, other)))
        }
        (Value::Number(a), Value::Number(b)) => {
            let integer = |n: &serde_json::Number| {
                n.as_i64()
                    .map(i128::from)
                    .or_else(|| n.as_u64().map(i128::from))
            };
            let (Some(i), Some(f)) = (
                integer(a).or_else(|| integer(b)),
                if integer(a).is_some() {
                    b.as_f64()
                } else {
                    a.as_f64()
                },
            ) else {
                return false;
            };
            // Both integer representations were already compared above.
            if integer(a).is_some() && integer(b).is_some() {
                return false;
            }
            f.is_finite()
                && f.fract() == 0.0
                && (-9223372036854775808.0..18446744073709551616.0).contains(&f)
                && f as i128 == i
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_scoring_preserves_large_integer_precision() {
        assert!(json_equal(
            &serde_json::json!([4.0, -2.0]),
            &serde_json::json!([4, -2])
        ));
        assert!(!json_equal(
            &serde_json::json!(1000000000000000000.0),
            &serde_json::json!(1000000000000000001u64)
        ));
        assert!(!json_equal(&serde_json::json!(1.5), &serde_json::json!(1)));
        assert!(!json_equal(&serde_json::json!(true), &serde_json::json!(1)));
    }
    #[test]
    fn pins_image_and_requires_isolation_flags() {
        assert!(validate_image("python:latest").is_err());
        assert!(validate_image("--evil@sha256:123").is_err());
        let args = docker_args("image", "mount", "name");
        for flag in [
            "--network=none",
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--user=65534:65534",
            "--pids-limit=32",
            "--memory=128m",
            "--pull=never",
        ] {
            assert!(args.iter().any(|a| a == flag));
        }
    }

    #[test]
    fn malformed_and_incorrect_results_never_pass() {
        let cases = vec![Case {
            name: "test".into(),
            input: serde_json::json!([]),
            expected: Some(serde_json::json!([])),
            error: None,
        }];
        assert!(score_output(b"[]", &cases).is_err());
        assert!(score_output(b"[{}]", &cases).is_err());
        assert!(score_output(b"[]\n[]", &cases).is_err());
        assert_eq!(
            score_output(br#"[{"result": [[1,2]], "input_after": []}]"#, &cases).unwrap(),
            0
        );
        assert_eq!(
            score_output(br#"[{"result": [], "input_after": []}]"#, &cases).unwrap(),
            1
        );
        assert_eq!(
            score_output(br#"[{"result": [], "input_after": [[1,2]]}]"#, &cases).unwrap(),
            0
        );
        let error_case = vec![Case {
            name: "invalid".into(),
            input: serde_json::json!([[5, 3]]),
            expected: None,
            error: Some("ValueError".into()),
        }];
        assert_eq!(
            score_output(
                br#"[{"error": "ValueError", "input_after": [[5,3]]}]"#,
                &error_case
            )
            .unwrap(),
            1
        );
        assert_eq!(
            score_output(
                br#"[{"error": "CandidateLoadError", "detail": "ValueError", "input_after": [[5,3]]}]"#,
                &error_case
            )
            .unwrap(),
            0
        );
    }
}
