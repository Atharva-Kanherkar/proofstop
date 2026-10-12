use crate::{
    candidate, inference, sandbox,
    task::{Config, Task, hash},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

struct Ledger {
    file: File,
    start: Instant,
}
impl Ledger {
    fn event(&mut self, kind: &str, fields: Value) -> Result<()> {
        serde_json::to_writer(
            &mut self.file,
            &json!({"event":kind,"elapsed_ms":self.start.elapsed().as_millis(),"fields":fields}),
        )?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    pub schema_version: u32,
    pub policy: String,
    pub evidence_class: String,
    pub outcome: String,
    pub task_id: String,
    pub task_revision: String,
    pub wall_ms: u128,
    pub allocated_gpu_seconds: Option<f64>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub acceptance: Option<Value>,
    pub evaluation: Option<Value>,
    pub error: Option<String>,
}

/// One generation request. There is deliberately no adaptive or early-exit policy yet.
pub async fn baseline(
    task: &Task,
    config: &Config,
    endpoint: &str,
    model: &str,
    output: &Path,
    evidence_class: &str,
) -> Result<Summary> {
    let url = inference::endpoint(endpoint)?;
    ensure!(!model.trim().is_empty(), "model must not be empty");
    ensure!(
        ["harness_validation", "unclassified"].contains(&evidence_class),
        "unsupported evidence class"
    );
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?; // Refuse to overwrite even an empty previous run.
    let started = Instant::now();
    let mut ledger = Ledger {
        file: OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join("events.jsonl"))?,
        start: started,
    };
    let mut summary = Summary {
        schema_version: 1,
        policy: if config.enable_thinking == Some(false) {
            "nonthinking_baseline"
        } else {
            "full_reasoning_baseline"
        }
        .into(),
        evidence_class: evidence_class.into(),
        outcome: "failed".into(),
        task_id: task.manifest.id.clone(),
        task_revision: task.manifest.revision.clone(),
        wall_ms: 0,
        allocated_gpu_seconds: None,
        prompt_tokens: None,
        completion_tokens: None,
        acceptance: None,
        evaluation: None,
        error: None,
    };
    let result: Result<()> = async {
        fs::write(output.join("task-manifest.json"), serde_json::to_vec_pretty(&task.manifest)?)?;
        fs::write(output.join("config.json"), serde_json::to_vec_pretty(config)?)?;
        let executable_hash = fs::read(std::env::current_exe()?).map(|b| hash(&b)).ok();
        let lock_hash = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock")).map(|b| hash(&b)).ok();
        ledger.event("run_started", json!({"unix_ms":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),"endpoint":url.as_str(),"requested_model":model,"binary_sha256":executable_hash,"cargo_lock_sha256":lock_hash,"engine_identity":null,"model_revision":null,"gpu_identity":null,"evidence_class":evidence_class}))?;
        let request = inference::request(model, &task.prompt()?, config);
        fs::write(output.join("request.json"), &request)?;
        ledger.event("inference_started", json!({"request_sha256":hash(&request)}))?;
        let wire = inference::send(url, request, config.request_timeout_seconds).await?;
        fs::write(output.join("response.json"), &wire.body)?;
        ledger.event("inference_finished", json!({"http_status":wire.status,"wall_ms":wire.wall_ms,"response_sha256":hash(&wire.body)}))?;
        ensure!((200..300).contains(&wire.status), "inference HTTP status {}", wire.status);
        // Even truncated completions incurred cost. Preserve reported usage
        // before validating whether the answer is eligible for execution.
        let envelope: Value = serde_json::from_slice(&wire.body)?;
        summary.prompt_tokens = envelope["usage"]["prompt_tokens"].as_u64();
        summary.completion_tokens = envelope["usage"]["completion_tokens"].as_u64();
        ledger.event("provider_usage_reported", json!({"usage":envelope.get("usage")}))?;
        let completion = inference::parse(&wire.body)?;
        ledger.event("completion_parsed", json!({"serving_model":completion.serving_model,"finish_reason":completion.finish_reason,"prompt_tokens":completion.prompt_tokens,"completion_tokens":completion.completion_tokens}))?;
        let source = candidate::extract(&completion.text)?;
        fs::write(output.join("candidate.py"), &source)?;
        ledger.event("candidate_frozen", json!({"sha256":hash(source.as_bytes())}))?;
        let acceptance = sandbox::verify(&source, task.cases("acceptance")?, config).await?;
        summary.acceptance = Some(serde_json::to_value(&acceptance)?);
        ledger.event("acceptance_finished", serde_json::to_value(&acceptance)?)?;
        // Evaluation happens after the candidate is frozen and never informs generation.
        let evaluation = sandbox::verify(&source, task.cases("evaluation")?, config).await?;
        summary.evaluation = Some(serde_json::to_value(&evaluation)?);
        ledger.event("evaluation_finished", serde_json::to_value(&evaluation)?)?;
        summary.outcome = match (&acceptance.verdict, &evaluation.verdict) {
            (sandbox::Verdict::Passed, sandbox::Verdict::Passed) => "passed",
            (sandbox::Verdict::Inconclusive, _) | (_, sandbox::Verdict::Inconclusive) => "inconclusive",
            _ => "failed_checks",
        }.into();
        Ok(())
    }.await;
    if let Err(error) = result {
        summary.error = Some(error.to_string());
        ledger.event("run_error", json!({"message":summary.error}))?;
    }
    summary.wall_ms = started.elapsed().as_millis();
    ledger.event("run_finished", serde_json::to_value(&summary)?)?;
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(summary)
}
