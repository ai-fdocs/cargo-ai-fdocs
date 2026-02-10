use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, AiFdocsError>;

#[derive(Debug, Error)]
pub enum AiFdocsError {
    #[error("config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("failed to read config file {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse TOML config {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },
}
