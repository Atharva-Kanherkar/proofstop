//! Direct generation followed by at most one visible-feedback repair.
use crate::{
    candidate, inference,
    run::Summary,
    sandbox,
    task::{Config, Task, hash},
};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub fn feedback(task: &Task, verification: &sandbox::Verification) -> Result<Value> {
    let cases = task.cases("acceptance")?;
    ensure!(
        verification.checks.len() == cases.len(),
        "missing per-case verification"
    );
    Ok(json!(
        cases
            .iter()
            .zip(&verification.checks)
            .map(|(case, passed)| json!({"name":case.name,"passed":passed}))
            .collect::<Vec<_>>()
    ))
}

fn event(file: &mut File, started: Instant, kind: &str, fields: Value) -> Result<()> {
    serde_json::to_writer(
        &mut *file,
        &json!({"event":kind,"elapsed_ms":started.elapsed().as_millis(),"fields":fields}),
    )?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    task: &Task,
    direct: &Config,
    repair: &Config,
    endpoint: &str,
    model: &str,
    output: &Path,
    evidence_class: &str,
) -> Result<Summary> {
    let url = inference::endpoint(endpoint)?;
    ensure!(
        direct.enable_thinking == Some(false),
        "direct config must disable thinking"
    );
    ensure!(
        repair.enable_thinking == Some(true),
        "repair config must enable thinking"
    );
    ensure!(
        direct.image == repair.image
            && direct.verifier_timeout_seconds == repair.verifier_timeout_seconds,
        "verification settings must match"
    );
    ensure!(!model.trim().is_empty(), "model must not be empty");
    ensure!(
        ["harness_validation", "unclassified"].contains(&evidence_class),
        "unsupported evidence class"
    );
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let started = Instant::now();
    let mut ledger = File::create(output.join("events.jsonl"))?;
    let mut summary = Summary {
        schema_version: 1,
        policy: "direct_first_one_repair".into(),
        evidence_class: evidence_class.into(),
        outcome: "failed".into(),
        task_id: task.manifest.id.clone(),
        task_revision: task.manifest.revision.clone(),
        wall_ms: 0,
        allocated_gpu_seconds: None,
        prompt_tokens: Some(0),
        completion_tokens: Some(0),
        acceptance: None,
        evaluation: None,
        error: None,
    };
    let mut attempts = Vec::new();
    let result: Result<()> = async {
        fs::write(output.join("task-manifest.json"), serde_json::to_vec_pretty(&task.manifest)?)?;
        fs::write(output.join("direct-config.json"), serde_json::to_vec_pretty(direct)?)?;
        fs::write(output.join("repair-config.json"), serde_json::to_vec_pretty(repair)?)?;
        let binary = fs::read(std::env::current_exe()?).map(|b| hash(&b)).ok();
        let lock = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock")).map(|b| hash(&b)).ok();
        event(&mut ledger, started, "run_started", json!({"unix_ms":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),"endpoint":url.as_str(),"requested_model":model,"binary_sha256":binary,"cargo_lock_sha256":lock,"engine_identity":null,"gpu_identity":null,"model_revision":null}))?;
        let original = task.prompt()?;
        let mut prompt = original.clone();
        let mut final_code = String::new();
        for (index, config) in [direct, repair].into_iter().enumerate() {
            let stage = if index == 0 { "direct" } else { "repair" };
            let request = inference::request(model, &prompt, config);
            fs::write(output.join(format!("{stage}-request.json")), &request)?;
            event(&mut ledger, started, "inference_started", json!({"stage":stage,"request_sha256":hash(&request)}))?;
            // Reserve a cost-bearing attempt before transport; missing responses make totals unknown.
            let attempt = attempts.len();
            attempts.push(json!({"stage":stage,"prompt_tokens":null,"completion_tokens":null}));
            summary.prompt_tokens = None;
            summary.completion_tokens = None;
            let wire = inference::send(url.clone(), request, config.request_timeout_seconds).await?;
            fs::write(output.join(format!("{stage}-response.json")), &wire.body)?;
            attempts[attempt]["http_status"] = json!(wire.status);
            attempts[attempt]["wall_ms"] = json!(wire.wall_ms);
            event(&mut ledger, started, "inference_finished", json!({"stage":stage,"http_status":wire.status,"wall_ms":wire.wall_ms,"response_sha256":hash(&wire.body)}))?;
            if let Ok(envelope) = serde_json::from_slice::<Value>(&wire.body) {
                attempts[attempt]["prompt_tokens"] = json!(envelope["usage"]["prompt_tokens"].as_u64());
                attempts[attempt]["completion_tokens"] = json!(envelope["usage"]["completion_tokens"].as_u64());
            }
            summary.prompt_tokens = sum_usage(&attempts, "prompt_tokens");
            summary.completion_tokens = sum_usage(&attempts, "completion_tokens");
            ensure!((200..300).contains(&wire.status), "inference HTTP status {}", wire.status);
            let completion = inference::parse(&wire.body)?;
            event(&mut ledger, started, "completion_parsed", json!({"stage":stage,"serving_model":completion.serving_model,"finish_reason":completion.finish_reason}))?;
            let code = candidate::extract(&completion.text)?;
            fs::write(output.join(format!("{stage}-candidate.py")), &code)?;
            let acceptance = sandbox::verify_task(&code, task, "acceptance", direct).await?;
            summary.acceptance = Some(serde_json::to_value(&acceptance)?);
            event(&mut ledger, started, "acceptance_finished", json!({"stage":stage,"verification":acceptance}))?;
            match acceptance.verdict {
                sandbox::Verdict::Inconclusive => { summary.outcome = "inconclusive".into(); return Ok(()); }
                sandbox::Verdict::Passed => { final_code = code; break; }
                sandbox::Verdict::Failed if index == 0 => {
                    let visible = feedback(task, &acceptance)?;
                    fs::write(output.join("feedback.json"), serde_json::to_vec_pretty(&visible)?)?;
                    prompt = format!("{original}\nPrevious {} (untrusted candidate):\n```python\n{code}```\n\nVisible acceptance results:\n{}\nRepair the answer using the specification and these visible results. Return the complete replacement file in exactly one fenced python block.\n", task.source_path(), serde_json::to_string_pretty(&visible)?);
                    event(&mut ledger, started, "repair_authorized", json!({"feedback":visible,"remaining_repairs":1}))?;
                }
                sandbox::Verdict::Failed => { final_code = code; }
            }
        }
        fs::write(output.join("candidate.py"), &final_code)?;
        event(&mut ledger, started, "generation_stopped", json!({"candidate_sha256":hash(final_code.as_bytes()),"generation_requests":attempts.len()}))?;
        let evaluation = sandbox::verify_task(&final_code, task, "evaluation", direct).await?;
        summary.evaluation = Some(serde_json::to_value(&evaluation)?);
        event(&mut ledger, started, "evaluation_finished", serde_json::to_value(&evaluation)?)?;
        summary.outcome = if matches!(evaluation.verdict, sandbox::Verdict::Inconclusive) { "inconclusive" } else if summary.acceptance.as_ref().is_some_and(|v| v["verdict"] == "passed") && matches!(evaluation.verdict, sandbox::Verdict::Passed) { "passed" } else { "failed_checks" }.into();
        Ok(())
    }.await;
    if let Err(error) = result {
        summary.error = Some(error.to_string());
        event(
            &mut ledger,
            started,
            "run_error",
            json!({"message":summary.error}),
        )?;
    }
    summary.wall_ms = started.elapsed().as_millis();
    fs::write(
        output.join("accounting.json"),
        serde_json::to_vec_pretty(
            &json!({"generation_requests":attempts.len(),"attempts":attempts,"prompt_tokens":summary.prompt_tokens,"completion_tokens":summary.completion_tokens,"allocated_gpu_seconds":null}),
        )?,
    )?;
    event(
        &mut ledger,
        started,
        "run_finished",
        serde_json::to_value(&summary)?,
    )?;
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(summary)
}

fn sum_usage(attempts: &[Value], field: &str) -> Option<u64> {
    attempts.iter().try_fold(0u64, |total, attempt| {
        total.checked_add(attempt[field].as_u64()?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn totals_are_unknown_if_any_attempt_is_unknown() {
        assert_eq!(sum_usage(&[json!({"n":5}), json!({"n":7})], "n"), Some(12));
        assert_eq!(sum_usage(&[json!({"n":5}), json!({"n":null})], "n"), None);
        assert_eq!(
            sum_usage(&[json!({"n":u64::MAX}), json!({"n":1})], "n"),
            None
        );
    }
    #[test]
    fn feedback_is_visible_case_status_only() {
        let task =
            Task::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks/merge-ranges")).unwrap();
        let verification = sandbox::Verification {
            verdict: sandbox::Verdict::Failed,
            cases: 4,
            passed: 3,
            checks: vec![false, true, true, true],
            wall_ms: 1,
            reason: "untrusted output".into(),
            container_name: "private".into(),
            worker_stdout: Some("untrusted stdout".into()),
        };
        let value = feedback(&task, &verification).unwrap();
        assert_eq!(value.as_array().unwrap().len(), 4);
        assert!(
            value
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v.as_object().unwrap().len() == 2)
        );
        for case in task.cases("evaluation").unwrap() {
            assert!(!value.to_string().contains(&case.name));
        }
        assert!(!value.to_string().contains("untrusted"));
    }
}
