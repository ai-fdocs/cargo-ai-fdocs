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

#[derive(Debug, Deserialize)]
pub struct Settings {
    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,

    #[serde(default = "default_max_file_size_kb")]
    pub max_file_size_kb: usize,

    #[serde(default = "default_true")]
    pub prune: bool,
}

#[derive(Debug, Deserialize)]
pub struct CrateDoc {
    pub sources: Vec<Source>,

    #[serde(default)]
    pub ai_notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Source {
    GitHub {
        repo: String,
        #[serde(default)]
        files: Vec<String>,
    },
    DocsRs,
}

#[derive(Debug, Deserialize)]
pub struct CrateSource {
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub repo: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Github,
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("docs/ai/vendor-docs")
}

fn default_max_file_size_kb() -> usize {
    200
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            max_file_size_kb: default_max_file_size_kb(),
            prune: default_true(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(AiDocsError::ConfigNotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        for (crate_name, crate_cfg) in &self.crates {
            if crate_cfg.sources.is_empty() {
                return Err(AiDocsError::InvalidConfig(format!(
                    "crate '{crate_name}' must define at least one source"
                )));
            }

            let has_github = crate_cfg
                .sources
                .iter()
                .any(|source| source.source_type == SourceType::Github);
            if !has_github {
                return Err(AiDocsError::InvalidConfig(format!(
                    "crate '{crate_name}' must define a source with type = 'github'"
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
    fn readme_example_parses_with_config_load() {
        let path = Path::new("examples/ai-docs.toml");
        let config = Config::load(path).expect("README example must parse");

        assert!(config.crates.contains_key("serde"));
        assert!(config.crates.contains_key("sqlx"));
    }

    #[test]
    fn config_without_sources_fails_validation() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-invalid-{suffix}.toml"));

        fs::write(&path, "[crates.serde]\nai_notes = \"x\"\n")
            .expect("must write temporary config");

        let err = Config::load(&path).expect_err("config without sources must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err.to_string().contains("must define at least one source"));
    }
}
