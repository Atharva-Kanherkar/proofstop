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
    repository: BTreeMap<String, Vec<u8>>,
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl Task {
    pub fn load(root: &Path) -> Result<Self> {
        let manifest: Manifest = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
        let source_file = match manifest.schema_version {
            1 => {
                ensure!(
                    manifest.id == "merge-ranges"
                        && ["synthetic-v1", "synthetic-v2"].contains(&manifest.revision.as_str()),
                    "unsupported task identity"
                );
                "repo/ranges.py"
            }
            2 => {
                ensure!(
                    !manifest.id.is_empty()
                        && manifest.id.len() <= 64
                        && manifest
                            .id
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                    "unsafe task identity"
                );
                ensure!(
                    manifest.revision == "synthetic-v1",
                    "unsupported task revision"
                );
                "repo/solution.py"
            }
            3 => {
                ensure!(
                    manifest.id == "naturalsize-rollover"
                        && manifest.revision == "snapshot-7574e0c",
                    "unsupported repository task identity"
                );
                "repo/humanize/filesize.py"
            }
            _ => bail!("unsupported task schema"),
        };
        let mut required = vec![source_file, "spec.md", "acceptance.json", "evaluation.json"];
        if manifest.schema_version == 3 {
            required.extend([
                "upstream.json",
                "repo/LICENCE",
                "repo/humanize/__init__.py",
                "repo/humanize/i18n.py",
                "repo/humanize/lists.py",
                "repo/humanize/number.py",
                "repo/humanize/time.py",
                "repo/humanize/_version.py",
                "repo/humanize/py.typed",
            ]);
        }
        ensure!(
            manifest.files.len() == required.len()
                && required.iter().all(|p| manifest.files.contains_key(*p)),
            "unexpected task file set"
        );
        let mut pinned = BTreeMap::new();
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
            let bytes = fs::read(&path)?;
            ensure!(
                hash(&bytes) == *expected_hash,
                "task hash mismatch: {relative}"
            );
            pinned.insert(relative.clone(), bytes);
        }
        let mut repository = BTreeMap::new();
        if manifest.schema_version == 3 {
            let upstream: Value = serde_json::from_slice(&pinned["upstream.json"])?;
            ensure!(
                upstream["repository"] == "https://github.com/python-humanize/humanize"
                    && upstream["base_commit"] == "7574e0cc377d80db0d2756fc2c5e0b8799a6f8b4"
                    && upstream["target"] == source_file,
                "unsupported upstream identity"
            );
            for relative in manifest.files.keys().filter(|p| p.starts_with("repo/")) {
                let bytes = pinned[relative].clone();
                ensure!(bytes.len() <= 128 * 1024, "repository file too large");
                repository.insert(relative.clone(), bytes);
            }
            ensure!(
                repository.values().map(Vec::len).sum::<usize>() <= 512 * 1024,
                "repository snapshot too large"
            );
        }
        let load_cases = |name: &str| -> Result<Vec<Case>> {
            let cases: Vec<Case> = serde_json::from_slice(&pinned[name])?;
            ensure!(!cases.is_empty(), "empty suite");
            for case in &cases {
                ensure!(
                    case.expected.is_some() ^ case.error.is_some(),
                    "case must specify exactly one expected result or error"
                );
                ensure!(case.input.is_array(), "case input must be an array");
                if manifest.schema_version == 3 {
                    ensure!(
                        case.input.as_array().unwrap().len() == 4,
                        "naturalsize requires four positional arguments"
                    );
                }
            }
            Ok(cases)
        };
        Ok(Self {
            source: String::from_utf8(pinned[source_file].clone())?,
            spec: String::from_utf8(pinned["spec.md"].clone())?,
            acceptance: load_cases("acceptance.json")?,
            evaluation: load_cases("evaluation.json")?,
            manifest,
            repository,
        })
    }

    pub fn prompt(&self) -> Result<String> {
        let fence = if self.repository.is_empty() {
            "```"
        } else {
            "````"
        };
        let mut prompt = format!(
            "{}\n\nCurrent {}:\n{fence}python\n{}{fence}\n\nVisible acceptance examples:\n{}\n",
            self.spec,
            self.source_path(),
            self.source,
            serde_json::to_string_pretty(&self.acceptance)?
        );
        if !self.repository.is_empty() {
            let buffer = std::str::from_utf8(&self.repository["repo/humanize/i18n.py"])?;
            prompt.push_str(&format!(
                "\nDependency context humanize/i18n.py (read-only):\n{fence}python\n{buffer}{fence}\n"
            ));
        }
        Ok(prompt)
    }

    pub fn source_path(&self) -> &'static str {
        match self.manifest.schema_version {
            1 => "ranges.py",
            2 => "solution.py",
            _ => "humanize/filesize.py",
        }
    }
    pub fn repository(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.repository
    }
    pub fn entrypoint(&self) -> &'static str {
        match self.manifest.schema_version {
            1 => "merge_ranges",
            2 => "solve",
            _ => "humanize",
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_thinking: Option<bool>,
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
    fn schema_two_is_pinned_and_rejects_unsafe_identity() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks/dev/binary-search");
        let task = Task::load(&root).unwrap();
        assert_eq!(task.entrypoint(), "solve");
        assert!(task.prompt().unwrap().contains("Current solution.py"));
        for case in task.cases("evaluation").unwrap() {
            assert!(!task.prompt().unwrap().contains(&case.name));
        }
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("repo")).unwrap();
        for name in [
            "manifest.json",
            "repo/solution.py",
            "spec.md",
            "acceptance.json",
            "evaluation.json",
        ] {
            fs::copy(root.join(name), tmp.path().join(name)).unwrap();
        }
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(tmp.path().join("manifest.json")).unwrap()).unwrap();
        manifest["id"] = "../escape".into();
        fs::write(
            tmp.path().join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(Task::load(tmp.path()).is_err());
        fs::copy(root.join("manifest.json"), tmp.path().join("manifest.json")).unwrap();
        fs::write(tmp.path().join("repo/solution.py"), "tampered").unwrap();
        assert!(Task::load(tmp.path()).is_err());
    }

    #[test]
    fn repository_prompt_excludes_reference_and_pins_dependencies() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks/repository/naturalsize-rollover");
        let task = Task::load(&root).unwrap();
        assert_eq!(task.repository().len(), 9);
        let prompt = task.prompt().unwrap();
        assert!(prompt.contains("Current humanize/filesize.py"));
        for case in task.cases("evaluation").unwrap() {
            assert!(!prompt.contains(&format!(
                "\"name\": {}",
                serde_json::to_string(&case.name).unwrap()
            )));
        }
        let tmp = tempfile::tempdir().unwrap();
        for relative in task
            .manifest
            .files
            .keys()
            .chain(std::iter::once(&"manifest.json".to_string()))
        {
            let p = tmp.path().join(relative);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::copy(root.join(relative), p).unwrap();
        }
        fs::write(
            tmp.path().join("repo/humanize/i18n.py"),
            "changed dependency",
        )
        .unwrap();
        assert!(Task::load(tmp.path()).is_err());
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
