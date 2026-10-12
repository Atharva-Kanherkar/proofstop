//! Experimental Qwen3/vLLM token-prefix replay. No KV fork or RNG-state resume.
use crate::{
    candidate, inference,
    run::Summary,
    sandbox,
    task::{Config, Task, hash},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Options {
    pub segment_tokens: u32,
    pub probe_tokens: u32,
    pub max_probes: u32,
}
impl Options {
    pub fn validate(&self, config: &Config) -> Result<()> {
        ensure!(
            (1..=config.max_tokens).contains(&self.segment_tokens),
            "segment tokens must be within main budget"
        );
        ensure!(
            (1..=4096).contains(&self.probe_tokens),
            "probe tokens must be 1..4096"
        );
        ensure!(self.max_probes <= 16, "at most 16 probes");
        ensure!(
            config.enable_thinking == Some(true),
            "online policies require explicit thinking mode"
        );
        Ok(())
    }
}

struct Recorder {
    root: PathBuf,
    endpoint: String,
    timeout: u64,
    start: Instant,
    events: File,
    sequence: usize,
    calls: u32,
    prompt_tokens: u64,
    completion_tokens: u64,
    usage_unknown: bool,
}
impl Recorder {
    fn event(&mut self, event: &str, fields: Value) -> Result<()> {
        serde_json::to_writer(
            &mut self.events,
            &json!({"event":event,"elapsed_ms":self.start.elapsed().as_millis(),"fields":fields}),
        )?;
        self.events.write_all(b"\n")?;
        self.events.flush()?;
        Ok(())
    }
    async fn call(
        &mut self,
        stage: &str,
        route: &str,
        request: Value,
        generation: bool,
    ) -> Result<Value> {
        let mut url = inference::endpoint(&self.endpoint)?;
        url.set_path(route);
        self.sequence += 1;
        let name = format!("{:03}-{stage}", self.sequence);
        let body = serde_json::to_vec(&request)?;
        fs::write(self.root.join(format!("{name}.request.json")), &body)?;
        self.event(
            "request_started",
            json!({"name":name,"route":route,"request_sha256":hash(&body),"generation":generation}),
        )?;
        if generation {
            self.calls += 1;
        }
        let wire = match inference::send(url, body, self.timeout).await {
            Ok(wire) => wire,
            Err(error) => {
                if generation {
                    self.usage_unknown = true;
                }
                return Err(error);
            }
        };
        fs::write(self.root.join(format!("{name}.response.json")), &wire.body)?;
        self.event("request_finished", json!({"name":name,"http_status":wire.status,"wall_ms":wire.wall_ms,"response_sha256":hash(&wire.body)}))?;
        let value: Value = serde_json::from_slice(&wire.body).inspect_err(|_| {
            if generation {
                self.usage_unknown = true;
            }
        })?;
        if generation {
            match (
                value["usage"]["prompt_tokens"].as_u64(),
                value["usage"]["completion_tokens"].as_u64(),
            ) {
                (Some(p), Some(c)) => {
                    self.prompt_tokens = self
                        .prompt_tokens
                        .checked_add(p)
                        .ok_or_else(|| anyhow::anyhow!("usage overflow"))?;
                    self.completion_tokens = self
                        .completion_tokens
                        .checked_add(c)
                        .ok_or_else(|| anyhow::anyhow!("usage overflow"))?;
                }
                _ => self.usage_unknown = true,
            }
            self.event(
                "provider_usage_reported",
                json!({"name":name,"usage":value.get("usage")}),
            )?;
        }
        ensure!(
            (200..300).contains(&wire.status),
            "{stage} HTTP status {}",
            wire.status
        );
        Ok(value)
    }
    async fn text(&mut self, model: &str, tokens: &[u32]) -> Result<String> {
        let value = self
            .call(
                "detokenize",
                "/detokenize",
                json!({"model":model,"tokens":tokens}),
                false,
            )
            .await?;
        let text = value["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing detokenized prompt"))?;
        ensure!(text.len() <= 256 * 1024, "detokenized trace exceeds cap");
        Ok(text.into())
    }
}

pub fn ids(value: &Value) -> Result<Vec<u32>> {
    let values = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing token IDs"))?;
    ensure!(values.len() <= 65536, "token array exceeds cap");
    values
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| anyhow::anyhow!("invalid token ID"))
        })
        .collect()
}

pub fn completion(value: &Value, prompt: &[u32], limit: u32) -> Result<(Vec<u32>, bool)> {
    let choices = value["choices"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing choices"))?;
    ensure!(
        choices.len() == 1 && value["model"].as_str().is_some(),
        "expected one serving-model choice"
    );
    let choice = &choices[0];
    let stopped = match choice["finish_reason"].as_str() {
        Some("stop") => true,
        Some("length") => false,
        _ => anyhow::bail!("unsupported completion finish reason"),
    };
    ensure!(
        ids(&choice["prompt_token_ids"])? == prompt,
        "server changed exact prompt token IDs"
    );
    let tokens = ids(&choice["token_ids"])?;
    ensure!(
        !tokens.is_empty() && tokens.len() <= limit as usize,
        "invalid completion token count"
    );
    ensure!(
        stopped || tokens.len() == limit as usize,
        "short length-limited completion"
    );
    if let Some(count) = value["usage"]["completion_tokens"].as_u64() {
        ensure!(
            count == tokens.len() as u64,
            "completion usage disagrees with token IDs"
        );
    }
    if let Some(count) = value["usage"]["prompt_tokens"].as_u64() {
        ensure!(
            count == prompt.len() as u64,
            "prompt usage disagrees with token IDs"
        );
    }
    Ok((tokens, stopped))
}

fn generation(model: &str, prompt: &[u32], limit: u32, config: &Config) -> Value {
    let mut value: Value = serde_json::from_slice(&inference::request(model, "", config))
        .expect("config serialization");
    value.as_object_mut().unwrap().remove("messages");
    value
        .as_object_mut()
        .unwrap()
        .remove("chat_template_kwargs");
    value["prompt"] = json!(prompt);
    value["max_tokens"] = json!(limit);
    value["return_token_ids"] = json!(true);
    value["add_special_tokens"] = json!(false);
    value
}

pub fn branch(main: &[u32], suffix: &[u32]) -> Vec<u32> {
    main.iter().chain(suffix).copied().collect()
}

async fn score(
    recorder: &mut Recorder,
    summary: &mut Summary,
    source: &str,
    task: &Task,
    config: &Config,
    label: &str,
) -> Result<bool> {
    fs::write(recorder.root.join(format!("{label}.py")), source)?;
    recorder.event(
        "candidate_frozen",
        json!({"label":label,"sha256":hash(source.as_bytes())}),
    )?;
    let acceptance = sandbox::verify(source, task.cases("acceptance")?, config).await?;
    recorder.event(
        "acceptance_finished",
        json!({"label":label,"verification":acceptance}),
    )?;
    summary.acceptance = Some(serde_json::to_value(&acceptance)?);
    match acceptance.verdict {
        sandbox::Verdict::Inconclusive => {
            summary.acceptance = Some(serde_json::to_value(&acceptance)?);
            summary.outcome = "inconclusive".into();
            return Ok(true);
        }
        sandbox::Verdict::Failed => return Ok(false),
        sandbox::Verdict::Passed => {}
    }
    // Commit to the decision before consulting evaluation. No later request is permitted.
    fs::write(recorder.root.join("candidate.py"), source)?;
    summary.acceptance = Some(serde_json::to_value(&acceptance)?);
    recorder.event("generation_stopped_on_acceptance", json!({"label":label}))?;
    let evaluation = sandbox::verify(source, task.cases("evaluation")?, config).await?;
    recorder.event("evaluation_finished", serde_json::to_value(&evaluation)?)?;
    summary.evaluation = Some(serde_json::to_value(&evaluation)?);
    summary.outcome = match evaluation.verdict {
        sandbox::Verdict::Passed => "passed",
        sandbox::Verdict::Failed => "failed_checks",
        sandbox::Verdict::Inconclusive => "inconclusive",
    }
    .into();
    Ok(true)
}

pub async fn run(
    task: &Task,
    config: &Config,
    endpoint: &str,
    model: &str,
    output: &Path,
    options: &Options,
    evidence: &str,
) -> Result<Summary> {
    options.validate(config)?;
    inference::endpoint(endpoint)?;
    ensure!(!model.trim().is_empty(), "model must not be empty");
    ensure!(
        ["unclassified", "harness_validation"].contains(&evidence),
        "unsupported evidence class"
    );
    ensure!(
        model == "Qwen/Qwen3-8B" || evidence == "harness_validation",
        "the experimental probe adapter supports Qwen/Qwen3-8B only"
    );
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let start = Instant::now();
    let mut recorder = Recorder {
        root: output.into(),
        endpoint: endpoint.into(),
        timeout: config.request_timeout_seconds,
        start,
        events: File::create(output.join("events.jsonl"))?,
        sequence: 0,
        calls: 0,
        prompt_tokens: 0,
        completion_tokens: 0,
        usage_unknown: false,
    };
    let mut summary = Summary {
        schema_version: 1,
        policy: if options.max_probes == 0 {
            "segmented_no_probe"
        } else {
            "periodic_probe"
        }
        .into(),
        evidence_class: evidence.into(),
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
    let mut main_generated = Vec::new();
    let mut probe_count = 0;
    let mut probe_generated = 0;
    let result: Result<()> = async {
        fs::write(output.join("task-manifest.json"), serde_json::to_vec_pretty(&task.manifest)?)?;
        fs::write(output.join("config.json"), serde_json::to_vec_pretty(config)?)?;
        fs::write(output.join("policy.json"), serde_json::to_vec_pretty(options)?)?;
        recorder.event("run_started", json!({"unix_ms":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),"model":model,"endpoint":endpoint,"binary_sha256":hash(&fs::read(std::env::current_exe()?)?),"cargo_lock_sha256":hash(&fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock"))?),"continuation":"exact token-prefix replay; seed resets; no KV fork"}))?;
        let prompt = task.prompt()?;
        let initial = recorder.call("tokenize-main", "/tokenize", json!({"model":model,"messages":[{"role":"user","content":prompt}],"add_generation_prompt":true,"add_special_tokens":false,"chat_template_kwargs":{"enable_thinking":true}}), false).await?;
        let prompt_ids = ids(&initial["tokens"])?;
        ensure!(!prompt_ids.is_empty() && initial["count"].as_u64()==Some(prompt_ids.len() as u64), "invalid tokenization count");
        let context_limit = initial["max_model_len"].as_u64().ok_or_else(|| anyhow::anyhow!("missing context limit"))?;
        ensure!(prompt_ids.len() as u64 + config.max_tokens as u64 <= context_limit, "main budget exceeds context limit");
        let suffix = if options.max_probes>0 {
            let value = recorder.call("tokenize-probe-suffix", "/tokenize", json!({"model":model,"prompt":"\n</think>\n\n","add_special_tokens":false}), false).await?;
            let tokens = ids(&value["tokens"])?;
            ensure!(!tokens.is_empty() && value["count"].as_u64()==Some(tokens.len() as u64), "invalid suffix tokenization");
            tokens
        } else { Vec::new() };
        while main_generated.len() < config.max_tokens as usize {
            let main = branch(&prompt_ids, &main_generated);
            let remaining = config.max_tokens - main_generated.len() as u32;
            let limit = options.segment_tokens.min(remaining);
            let value = recorder.call("main", "/v1/completions", generation(model, &main, limit, config), true).await?;
            let (tokens, stopped) = completion(&value, &main, limit)?;
            main_generated.extend(tokens);
            let trace = recorder.text(model, &main_generated).await?;
            fs::write(output.join("main-trace.txt"), &trace)?;
            recorder.event("main_checkpoint", json!({"generated_tokens":main_generated.len(),"stopped":stopped,"prefix_sha256":hash(&serde_json::to_vec(&branch(&prompt_ids,&main_generated))?)}))?;
            if stopped {
                match candidate::extract(&format!("<think>\n{trace}")) {
                    Ok(source) => { if !score(&mut recorder, &mut summary, &source, task, config, "natural-answer").await? { summary.outcome="failed_checks".into(); } },
                    Err(error) => { summary.error=Some(error.to_string()); },
                }
                break;
            }
            // Do not probe after the main budget is exhausted.
            if probe_count < options.max_probes && main_generated.len() < config.max_tokens as usize {
                probe_count += 1;
                let main = branch(&prompt_ids, &main_generated);
                let suffix = if trace.contains("</think>") { &[][..] } else { suffix.as_slice() };
                let probe_prompt = branch(&main, suffix);
                ensure!(probe_prompt.len() as u64 + options.probe_tokens as u64 <= context_limit, "probe budget exceeds context limit");
                recorder.event("probe_started", json!({"probe":probe_count,"main_tokens":main_generated.len(),"main_prefix_sha256":hash(&serde_json::to_vec(&main)?),"probe_prefix_sha256":hash(&serde_json::to_vec(&probe_prompt)?),"suffix_tokens":suffix.len()}))?;
                let value = recorder.call("probe", "/v1/completions", generation(model, &probe_prompt, options.probe_tokens, config), true).await?;
                let (tokens, stopped) = completion(&value, &probe_prompt, options.probe_tokens)?;
                probe_generated += tokens.len();
                if !stopped { recorder.event("probe_rejected", json!({"probe":probe_count,"reason":"truncated"}))?; continue; }
                let text = recorder.text(model, &branch(&branch(&main_generated,suffix),&tokens)).await?;
                let label = format!("probe-{probe_count:02}");
                fs::write(output.join(format!("{label}.txt")), &text)?;
                match candidate::extract(&format!("<think>\n{text}")) {
                    Ok(source) => { if score(&mut recorder, &mut summary, &source, task, config, &label).await? { break; } },
                    Err(error) => { recorder.event("probe_rejected", json!({"probe":probe_count,"reason":error.to_string()}))?; },
                }
            }
        }
        if summary.outcome=="failed" && summary.error.is_none() { summary.error=Some("main budget exhausted without accepted final answer".into()); }
        Ok(())
    }.await;
    if let Err(error) = result {
        summary.error = Some(error.to_string());
        recorder.event("run_error", json!({"message":summary.error}))?;
    }
    summary.wall_ms = start.elapsed().as_millis();
    if !recorder.usage_unknown {
        summary.prompt_tokens = Some(recorder.prompt_tokens);
        summary.completion_tokens = Some(recorder.completion_tokens);
    }
    recorder.event("run_finished", serde_json::to_value(&summary)?)?;
    fs::write(
        output.join("accounting.json"),
        serde_json::to_vec_pretty(
            &json!({"generation_requests":recorder.calls,"all_http_requests":recorder.sequence,"reasoning_tokens":main_generated.len(),"probe_tokens":probe_generated,"probes":probe_count,"reported_prompt_tokens_known_sum":recorder.prompt_tokens,"reported_completion_tokens_known_sum":recorder.completion_tokens,"usage_unknown":recorder.usage_unknown,"replay_and_probe_usage_included":true,"allocated_gpu_seconds":null}),
        )?,
    )?;
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_branch_does_not_mutate_main() {
        let main = vec![1, 2, 3];
        assert_eq!(branch(&main, &[4, 5]), vec![1, 2, 3, 4, 5]);
        assert_eq!(main, vec![1, 2, 3]);
    }
    #[test]
    fn token_response_fails_closed() {
        let mut value = json!({"model":"m","choices":[{"finish_reason":"length","prompt_token_ids":[1,2],"token_ids":[3]}],"usage":{"prompt_tokens":2,"completion_tokens":1}});
        assert_eq!(completion(&value, &[1, 2], 1).unwrap(), (vec![3], false));
        assert!(completion(&value, &[1, 2], 2).is_err());
        assert!(completion(&value, &[1, 9], 2).is_err());
        value["choices"][0]["token_ids"] = json!([-1]);
        assert!(completion(&value, &[1, 2], 2).is_err());
        value["choices"][0]["token_ids"] = json!([3, 4, 5]);
        assert!(completion(&value, &[1, 2], 2).is_err());
        value["choices"][0]["token_ids"] = json!([3]);
        value["choices"][0]["finish_reason"] = json!("tool_calls");
        assert!(completion(&value, &[1, 2], 2).is_err());
    }
}
