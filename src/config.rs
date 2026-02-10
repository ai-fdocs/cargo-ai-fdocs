use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AiDocsError, Result};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,

    #[serde(default)]
    pub crates: HashMap<String, CrateConfig>,
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
pub struct CrateConfig {
    /// Source definitions (at least one is required).
    pub sources: Vec<CrateSource>,

    /// Optional: Explicit list of files to fetch.
    /// Paths are relative to repo root.
    pub files: Option<Vec<String>>,

    /// Instructions for AI (goes into _INDEX.md)
    #[serde(default)]
    pub ai_notes: String,
}

#[derive(Debug, Deserialize)]
pub struct CrateSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub repo: String,
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("docs/ai/vendor-docs/rust")
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
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::Config;

    #[test]
    fn readme_example_parses_with_config_load() {
        let path = Path::new("examples/ai-docs.toml");
        let config = Config::load(path).expect("README example must parse");

        assert!(config.crates.contains_key("serde"));
        assert!(config.crates.contains_key("sqlx"));
    }
}
