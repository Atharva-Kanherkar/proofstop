use crate::task::Config;
use anyhow::{Result, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const RESPONSE_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct Completion {
    pub text: String,
    pub serving_model: String,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub finish_reason: String,
}

#[derive(Debug)]
pub struct WireResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub wall_ms: u128,
}

pub fn request(model: &str, prompt: &str, config: &Config) -> Vec<u8> {
    serde_json::to_vec(&json!({"model":model,"messages":[{"role":"user","content":prompt}],"stream":false,"n":1,"max_tokens":config.max_tokens,"seed":config.seed,"temperature":config.temperature})).expect("validated configuration")
}

pub fn endpoint(base: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(base)?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "endpoint must be HTTP(S)"
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "endpoint must not contain credentials, query, or fragment"
    );
    ensure!(
        url.path().trim_end_matches('/') == "/v1",
        "endpoint must end in /v1"
    );
    Ok(reqwest::Url::parse(&format!(
        "{}/chat/completions",
        base.trim_end_matches('/')
    ))?)
}

pub async fn send(url: reqwest::Url, body: Vec<u8>, timeout_seconds: u64) -> Result<WireResponse> {
    let started = Instant::now();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut response = client
        .post(url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await?;
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= RESPONSE_LIMIT,
            "response exceeds 1 MiB capture limit"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(WireResponse {
        status,
        body: bytes,
        wall_ms: started.elapsed().as_millis(),
    })
}

pub fn parse(body: &[u8]) -> Result<Completion> {
    let value: Value = serde_json::from_slice(body)?;
    let choices = value["choices"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing choices"))?;
    ensure!(choices.len() == 1, "expected one completion");
    let choice = &choices[0];
    ensure!(
        choice["finish_reason"] == "stop",
        "completion did not finish normally"
    );
    ensure!(
        choice["message"]["tool_calls"].is_null(),
        "tool calls are unsupported"
    );
    let text = choice["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing content"))?
        .to_owned();
    ensure!(!text.trim().is_empty(), "empty completion");
    Ok(Completion {
        text,
        serving_model: value["model"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing serving model"))?
            .into(),
        prompt_tokens: value["usage"]["prompt_tokens"].as_u64(),
        completion_tokens: value["usage"]["completion_tokens"].as_u64(),
        finish_reason: "stop".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finish_reason_and_usage_are_not_guessed() {
        let mut value = json!({"model":"test","choices":[{"finish_reason":"stop","message":{"content":"```python\nx=1\n```"}}]});
        assert!(
            parse(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .completion_tokens
                .is_none()
        );
        value["choices"][0]["finish_reason"] = "length".into();
        assert!(parse(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(parse(br#"{"choices":[]}"#).is_err());
    }

    #[test]
    fn endpoint_rejects_embedded_credentials() {
        assert!(endpoint("http://localhost:8000/v1").is_ok());
        assert!(endpoint("http://user:secret@localhost/v1").is_err());
        assert!(endpoint("http://localhost/v1?api_key=secret").is_err());
        assert!(endpoint("http://localhost/other").is_err());
    }
}
