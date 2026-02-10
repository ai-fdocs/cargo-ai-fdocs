use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, AiDocsError>;

#[derive(Error, Debug)]
pub enum AiDocsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parsing error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("Config file not found at: {0}")]
    ConfigNotFound(PathBuf),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTTP request failed for {url}: {source}")]
    Fetch { url: String, source: reqwest::Error },

    #[error("Cargo.lock parsing error: {0}")]
    CargoLockParse(String),

    #[error("Cargo.lock not found. Please run 'cargo build' first.")]
    CargoLockNotFound,

    #[error("GitHub file not found: {repo} / {path} (refs tried: {tried_tags:?})")]
    GitHubFileNotFound {
        repo: String,
        path: String,
        tried_tags: Vec<String>,
    },

    #[error("Optional file not found: {0}")]
    OptionalFileNotFound(String),

    #[error("{0}")]
    Other(String),
}
