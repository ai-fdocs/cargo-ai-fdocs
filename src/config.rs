use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AiFdocsError, Result};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub crates: BTreeMap<String, CrateConfig>,
}

#[derive(Debug, Deserialize)]
pub struct CrateConfig {
    pub repo: String,
    #[serde(default)]
    pub subpath: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub ai_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,
    #[serde(default = "default_max_file_size_kb")]
    pub max_file_size_kb: usize,
    #[serde(default = "default_prune")]
    pub prune: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            max_file_size_kb: default_max_file_size_kb(),
            prune: default_prune(),
        }
    }
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("docs/ai/vendor-docs/rust")
}

fn default_max_file_size_kb() -> usize {
    200
}

fn default_prune() -> bool {
    true
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(AiFdocsError::ConfigNotFound(path.to_path_buf()));
        }

        let raw = std::fs::read_to_string(path).map_err(|source| AiFdocsError::ConfigRead {
            path: path.to_path_buf(),
            source,
        })?;

        toml::from_str(&raw).map_err(|source| AiFdocsError::ConfigParse {
            path: path.to_path_buf(),
            source,
        })
    }
}
