use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, AiDocsError>;

#[derive(Error, Debug)]
pub enum AiDocsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parsing error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Config file not found at: {0}")]
    ConfigNotFound(PathBuf),

    #[error("HTTP request failed for {url}: {source}")]
    Fetch { url: String, source: reqwest::Error },

    #[error("Cargo.lock parsing error: {0}")]
    CargoLockParse(String),

    #[error("Cargo.lock not found. Please run 'cargo build' first.")]
    CargoLockNotFound,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, AiDocsError>;
