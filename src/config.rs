use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AiDocsError, Result};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,

    #[serde(default)]
    pub crates: HashMap<String, CrateDoc>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DocsSource {
    #[serde(rename = "github")]
    GitHub,
}

const fn default_docs_source() -> DocsSource {
    DocsSource::GitHub
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,

    #[serde(default = "default_max_file_size_kb")]
    pub max_file_size_kb: usize,

    #[serde(default = "default_true")]
    pub prune: bool,

    #[serde(default = "default_sync_concurrency")]
    pub sync_concurrency: usize,

    #[serde(default = "default_docs_source")]
    pub docs_source: DocsSource,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CrateDoc {
    /// New format: explicit repository in crate section.
    pub repo: Option<String>,
    /// Optional subpath for monorepos (used for defaults only).
    pub subpath: Option<String>,
    /// Optional explicit file list.
    pub files: Option<Vec<String>>,

    /// Legacy format compatibility.
    pub sources: Option<Vec<Source>>,

    #[serde(default)]
    pub ai_notes: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Source {
    GitHub {
        repo: String,
        #[serde(default)]
        files: Vec<String>,
    },
    DocsRs,
}

impl CrateDoc {
    pub fn github_repo(&self) -> Option<&str> {
        if let Some(repo) = self.repo.as_deref() {
            return Some(repo);
        }

        self.sources.as_ref().and_then(|sources| {
            sources.iter().find_map(|s| match s {
                Source::GitHub { repo, .. } => Some(repo.as_str()),
                Source::DocsRs => None,
            })
        })
    }

    pub fn effective_files(&self) -> Option<Vec<String>> {
        if let Some(files) = &self.files {
            return Some(files.clone());
        }

        self.sources.as_ref().and_then(|sources| {
            sources.iter().find_map(|s| match s {
                Source::GitHub { files, .. } if !files.is_empty() => Some(files.clone()),
                _ => None,
            })
        })
    }
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("docs/ai/vendor-docs")
}

const fn default_max_file_size_kb() -> usize {
    200
}

const fn default_true() -> bool {
    true
}

const fn default_sync_concurrency() -> usize {
    8
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            max_file_size_kb: default_max_file_size_kb(),
            prune: default_true(),
            sync_concurrency: default_sync_concurrency(),
            docs_source: default_docs_source(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(AiDocsError::ConfigNotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.settings.sync_concurrency == 0 {
            return Err(AiDocsError::InvalidConfig(
                "settings.sync_concurrency must be greater than 0".to_string(),
            ));
        }

        for (crate_name, crate_cfg) in &self.crates {
            if crate_cfg.github_repo().is_none() {
                return Err(AiDocsError::InvalidConfig(format!(
                    "crate '{crate_name}' must define `repo` or legacy `sources` with GitHub"
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::Config;

    #[test]
    fn example_config_parses_with_config_load() {
        let path = Path::new("examples/ai-docs.toml");
        let config = Config::load(path).expect("example config must parse");

        assert!(config.crates.contains_key("serde"));
        assert!(config.crates.contains_key("sqlx"));
    }

    #[test]
    fn config_with_zero_sync_concurrency_fails_validation() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ai-fdocs-invalid-sync-concurrency-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
sync_concurrency = 0

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("zero sync_concurrency must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("settings.sync_concurrency must be greater than 0"));
    }

    #[test]
    fn config_with_invalid_docs_source_fails_parse() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-invalid-docs-source-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
docs_source = "npm_tarball"

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("invalid docs_source must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(
            err.to_string().contains("unknown variant") || err.to_string().contains("docs_source")
        );
    }

    #[test]
    fn config_without_docs_source_uses_github_default() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-default-docs-source-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
sync_concurrency = 2

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let cfg = Config::load(&path).expect("config without docs_source should parse");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(matches!(
            cfg.settings.docs_source,
            super::DocsSource::GitHub
        ));
    }
    #[test]
    fn config_without_repo_or_sources_fails_validation() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-invalid-{suffix}.toml"));

        fs::write(&path, "[crates.serde]\nai_notes = \"x\"\n")
            .expect("must write temporary config");

        let err = Config::load(&path).expect_err("config without repo/sources must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("must define `repo` or legacy `sources` with GitHub"));
    }
}
