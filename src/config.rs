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
