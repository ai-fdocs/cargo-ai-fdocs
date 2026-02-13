use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, AiDocsError>;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SyncErrorKind {
    Auth,
    RateLimit,
    Network,
    NotFound,
    Other,
}

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

    #[error("GitHub authentication failed for {url}: HTTP {status}")]
    GitHubAuth { url: String, status: u16 },

    #[error("GitHub API rate limit exceeded for {url}: HTTP {status}. Set GITHUB_TOKEN/GH_TOKEN.")]
    GitHubRateLimit { url: String, status: u16 },

    #[error("HTTP request failed for {url}: status {status}")]
    HttpStatus { url: String, status: u16 },

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

impl AiDocsError {
    pub fn sync_kind(&self) -> SyncErrorKind {
        match self {
            Self::GitHubAuth { .. } => SyncErrorKind::Auth,
            Self::GitHubRateLimit { .. } => SyncErrorKind::RateLimit,
            Self::Http(_) | Self::Fetch { .. } => SyncErrorKind::Network,
            Self::GitHubFileNotFound { .. } | Self::OptionalFileNotFound(_) => {
                SyncErrorKind::NotFound
            }
            Self::HttpStatus { status, .. } if *status == 404 => SyncErrorKind::NotFound,
            Self::HttpStatus { status, .. } if *status >= 500 => SyncErrorKind::Network,
            _ => SyncErrorKind::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AiDocsError, SyncErrorKind};

    #[test]
    fn maps_error_kinds_for_sync_summary() {
        assert_eq!(
            AiDocsError::GitHubAuth {
                url: "u".to_string(),
                status: 401
            }
            .sync_kind(),
            SyncErrorKind::Auth
        );
        assert_eq!(
            AiDocsError::GitHubRateLimit {
                url: "u".to_string(),
                status: 429
            }
            .sync_kind(),
            SyncErrorKind::RateLimit
        );
        assert_eq!(
            AiDocsError::HttpStatus {
                url: "u".to_string(),
                status: 404
            }
            .sync_kind(),
            SyncErrorKind::NotFound
        );
        assert_eq!(
            AiDocsError::Other("x".to_string()).sync_kind(),
            SyncErrorKind::Other
        );
    }
}
