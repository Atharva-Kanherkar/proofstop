use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub name: String,
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub revision: String,
    pub files: BTreeMap<String, String>,
}

pub struct Task {
    pub manifest: Manifest,
    pub source: String,
    spec: String,
    pub acceptance: Vec<Case>,
    evaluation: Vec<Case>,
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl Task {
    pub fn load(root: &Path) -> Result<Self> {
        let manifest: Manifest = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
        ensure!(manifest.schema_version == 1, "unsupported task schema");
        ensure!(
            manifest.id == "merge-ranges" && manifest.revision == "synthetic-v1",
            "unsupported task identity"
        );
        let required = [
            "repo/ranges.py",
            "spec.md",
            "acceptance.json",
            "evaluation.json",
        ];
        ensure!(
            manifest.files.len() == required.len()
                && required.iter().all(|p| manifest.files.contains_key(*p)),
            "unexpected task file set"
        );
        for (relative, expected_hash) in &manifest.files {
            ensure!(
                Path::new(relative)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
                "unsafe task path"
            );
            let path = root.join(relative);
            for ancestor in path.ancestors().take_while(|p| *p != root) {
                ensure!(
                    !fs::symlink_metadata(ancestor)?.file_type().is_symlink(),
                    "task files cannot use symlinks"
                );
            }
            ensure!(
                hash(&fs::read(&path)?) == *expected_hash,
                "task hash mismatch: {relative}"
            );
        }
        let load_cases = |name: &str| -> Result<Vec<Case>> {
            let cases: Vec<Case> = serde_json::from_slice(&fs::read(root.join(name))?)?;
            ensure!(!cases.is_empty(), "empty suite");
            for case in &cases {
                ensure!(
                    case.expected.is_some() ^ case.error.is_some(),
                    "case must specify exactly one expected result or error"
                );
                ensure!(case.input.is_array(), "case input must be an array");
            }
            Ok(cases)
        };
        Ok(Self {
            source: fs::read_to_string(root.join("repo/ranges.py"))?,
            spec: fs::read_to_string(root.join("spec.md"))?,
            acceptance: load_cases("acceptance.json")?,
            evaluation: load_cases("evaluation.json")?,
            manifest,
        })
    }

    pub fn prompt(&self) -> Result<String> {
        Ok(format!(
            "{}\n\nCurrent ranges.py:\n```python\n{}```\n\nVisible acceptance examples:\n{}\n",
            self.spec,
            self.source,
            serde_json::to_string_pretty(&self.acceptance)?
        ))
    }

    pub fn cases(&self, suite: &str) -> Result<&[Case]> {
        match suite {
            "acceptance" => Ok(&self.acceptance),
            "evaluation" => Ok(&self.evaluation),
            _ => bail!("unknown suite: {suite}"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub image: String,
    pub verifier_timeout_seconds: u64,
    pub request_timeout_seconds: u64,
    pub max_tokens: u32,
    pub seed: u64,
    pub temperature: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let config: Self = serde_json::from_slice(&fs::read(path).context("read config")?)?;
        ensure!(config.schema_version == 1, "unsupported config schema");
        ensure!(
            (1..=60).contains(&config.verifier_timeout_seconds),
            "verifier timeout must be 1..60 seconds"
        );
        ensure!(
            (1..=600).contains(&config.request_timeout_seconds),
            "request timeout must be 1..600 seconds"
        );
        ensure!(
            (1..=32768).contains(&config.max_tokens),
            "max_tokens must be 1..32768"
        );
        ensure!(
            config.temperature.is_finite() && (0.0..=2.0).contains(&config.temperature),
            "invalid temperature"
        );
        crate::sandbox::validate_image(&config.image)?;
        ensure!(
            config
                .top_p
                .is_none_or(|p| p.is_finite() && p > 0.0 && p <= 1.0),
            "top_p must be in (0, 1]"
        );
        ensure!(
            config.top_k.is_none_or(|k| k == -1 || k > 0),
            "top_k must be -1 or positive"
        );
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampling_values_are_validated() {
        let original: Value = serde_json::from_slice(
            &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("configs/local.json")).unwrap(),
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        for (p, k, valid) in [
            (0.95, 20, true),
            (1.0, -1, true),
            (0.0, 20, false),
            (-0.1, 20, false),
            (1.1, 20, false),
            (0.95, 0, false),
            (0.95, -2, false),
        ] {
            let mut value = original.clone();
            value["top_p"] = serde_json::json!(p);
            value["top_k"] = serde_json::json!(k);
            fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            assert_eq!(Config::load(&path).is_ok(), valid, "top_p={p}, top_k={k}");
        }
    }
    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks/merge-ranges")
    }

    #[test]
    fn fixture_is_pinned_and_evaluation_is_not_in_prompt() {
        let task = Task::load(&fixture()).unwrap();
        let prompt = task.prompt().unwrap();
        assert!(prompt.contains("nested_range"));
        for case in task.cases("evaluation").unwrap() {
            assert!(!prompt.contains(&case.name));
        }
        assert!(!prompt.contains("1000000000000000000"));
    }

    #[test]
    fn changed_source_and_unsupported_manifest_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("repo")).unwrap();
        for name in [
            "manifest.json",
            "repo/ranges.py",
            "spec.md",
            "acceptance.json",
            "evaluation.json",
        ] {
            fs::copy(fixture().join(name), tmp.path().join(name)).unwrap();
        }
        fs::write(tmp.path().join("repo/ranges.py"), "tampered").unwrap();
        assert!(Task::load(tmp.path()).is_err());
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(tmp.path().join("manifest.json")).unwrap()).unwrap();
        manifest["schema_version"] = 99.into();
        fs::write(
            tmp.path().join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(Task::load(tmp.path()).is_err());
    }
}
