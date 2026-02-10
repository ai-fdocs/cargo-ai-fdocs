use std::env;

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::error::{AiDocsError, Result};

const APP_USER_AGENT: &str = concat!("cargo-ai-fdocs/", env!("CARGO_PKG_VERSION"));

pub struct GitHubFetcher {
    client: Client,
    pub token_present: bool,
}

#[derive(Debug)]
pub struct ResolvedRef {
    pub git_ref: String,
    pub is_fallback: bool,
}

#[derive(Deserialize)]
struct RepoInfo {
    default_branch: String,
}

impl GitHubFetcher {
    pub fn new() -> Result<Self> {
        let token = env::var("GITHUB_TOKEN")
            .or_else(|_| env::var("GH_TOKEN"))
            .ok();
        let token_present = token.is_some();

        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(t) = token {
            let mut auth_val = reqwest::header::HeaderValue::from_str(&format!("Bearer {t}"))
                .map_err(|_| AiDocsError::Unknown("Invalid token characters".to_string()))?;
            auth_val.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, auth_val);
        } else {
            warn!(
                "⚠ No GITHUB_TOKEN found. Rate limit is strict (60 req/hr). Set GITHUB_TOKEN for 5000 req/hr."
            );
        }

        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            token_present,
        })
    }

    /// Resolves a tag for the crate version. Falls back to default branch.
    pub async fn resolve_ref(
        &self,
        owner_repo: &str,
        crate_name: &str,
        version: &str,
    ) -> Result<ResolvedRef> {
        let candidates = vec![
            format!("v{version}"),
            version.to_string(),
            format!("{crate_name}-v{version}"),
            format!("{crate_name}-{version}"),
        ];

        for tag in candidates {
            let url = format!("https://api.github.com/repos/{owner_repo}/git/ref/tags/{tag}");
            debug!("Checking tag: {url}");

            let res = self.client.get(&url).send().await?;
            if res.status().is_success() {
                debug!("Found tag: {tag}");
                return Ok(ResolvedRef {
                    git_ref: tag,
                    is_fallback: false,
                });
            } else if res.status() == StatusCode::TOO_MANY_REQUESTS {
                return Err(AiDocsError::Unknown(
                    "GitHub API Rate Limit Exceeded".to_string(),
                ));
            }
        }

        warn!(
            "Tag for version {} not found in {}. Falling back to default branch.",
            version, owner_repo
        );

        let url = format!("https://api.github.com/repos/{owner_repo}");
        let repo_info: RepoInfo = self.client.get(&url).send().await?.json().await?;

        Ok(ResolvedRef {
            git_ref: repo_info.default_branch,
            is_fallback: true,
        })
    }

    /// Fetches file via raw.githubusercontent.com
    pub async fn fetch_file(
        &self,
        owner_repo: &str,
        git_ref: &str,
        path: &str,
    ) -> Result<Option<String>> {
        let url = format!("https://raw.githubusercontent.com/{owner_repo}/{git_ref}/{path}");
        debug!("Fetching file: {url}");

        let res = self.client.get(&url).send().await?;

        if res.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !res.status().is_success() {
            return Err(AiDocsError::Network(res.error_for_status().unwrap_err()));
        }

        let text = res.text().await?;
        Ok(Some(text))
    }
}
