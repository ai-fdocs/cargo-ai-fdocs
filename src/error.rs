use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiDocsError {
    #[error("Config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("Failed to parse config: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Cargo.lock not found at {0}")]
    CargoLockNotFound(PathBuf),

    #[error("Failed to parse Cargo.lock: {0}")]
    CargoLockParse(String),

    #[error("HTTP request failed for {url}: {source}")]
    Fetch {
        url: String,
        source: reqwest::Error,
    },

    #[error("GitHub file not found: {repo}/{path} (tried tags: {tried_tags:?})")]
    GitHubFileNotFound {
        repo: String,
        path: String,
        tried_tags: Vec<String>,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AiDocsError>;