use std::env;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::error::{AiDocsError, Result};

const APP_USER_AGENT: &str = concat!("cargo-ai-fdocs/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct ResolvedRef {
    pub git_ref: String,
    pub is_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct FetchedFile {
    pub path: String,
    pub source_url: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct FileRequest {
    pub original_path: String,
    pub candidates: Vec<String>,
    pub required: bool,
}

pub struct GitHubFetcher {
    client: Client,
}

#[derive(Deserialize)]
struct RepoInfo {
    default_branch: String,
}

impl GitHubFetcher {
    pub fn new() -> Self {
        let token = env::var("GITHUB_TOKEN")
            .or_else(|_| env::var("GH_TOKEN"))
            .ok();

        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(t) = token {
            if let Ok(mut auth_val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {t}"))
            {
                auth_val.set_sensitive(true);
                headers.insert(reqwest::header::AUTHORIZATION, auth_val);
            }
        } else {
            warn!(
                "⚠ No GITHUB_TOKEN found. Rate limit: 60 req/hr. Set GITHUB_TOKEN for 5000 req/hr."
            );
        }

        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");

        Self { client }
    }

    pub async fn resolve_ref(
        &self,
        owner_repo: &str,
        crate_name: &str,
        version: &str,
    ) -> Result<ResolvedRef> {
        let candidates = [
            format!("v{version}"),
            version.to_string(),
            format!("{crate_name}-v{version}"),
            format!("{crate_name}-{version}"),
        ];

        for tag in candidates {
            let url = format!("https://api.github.com/repos/{owner_repo}/git/ref/tags/{tag}");
            let res = self.send_with_retry(url.as_str()).await?;
            if res.status().is_success() {
                return Ok(ResolvedRef {
                    git_ref: tag,
                    is_fallback: false,
                });
            }
        }

        let repo_url = format!("https://api.github.com/repos/{owner_repo}");
        let repo_resp = self.send_with_retry(repo_url.as_str()).await?;
        if !repo_resp.status().is_success() {
            return Err(AiDocsError::Other(format!(
                "Failed to resolve default branch for {owner_repo}: {}",
                repo_resp.status()
            )));
        }

        let repo_info: RepoInfo = repo_resp.json().await?;

        Ok(ResolvedRef {
            git_ref: repo_info.default_branch,
            is_fallback: true,
        })
    }

    pub async fn fetch_files(
        &self,
        repo: &str,
        git_ref: &str,
        requests: &[FileRequest],
    ) -> Vec<Result<FetchedFile>> {
        let mut out = Vec::with_capacity(requests.len());
        for req in requests {
            out.push(self.fetch_file(repo, git_ref, req).await);
        }
        out
    }

    async fn fetch_file(
        &self,
        repo: &str,
        git_ref: &str,
        req: &FileRequest,
    ) -> Result<FetchedFile> {
        let mut tried = Vec::new();

        for candidate in &req.candidates {
            tried.push(candidate.clone());
            let url = format!("https://raw.githubusercontent.com/{repo}/{git_ref}/{candidate}");
            let res = self.send_with_retry(url.as_str()).await?;

            if res.status() == StatusCode::NOT_FOUND {
                continue;
            }

            if !res.status().is_success() {
                return Err(AiDocsError::Other(format!(
                    "Failed to fetch '{}' from {}@{}: {}",
                    candidate,
                    repo,
                    git_ref,
                    res.status()
                )));
            }

            let content = res.text().await?;
            return Ok(FetchedFile {
                path: req.original_path.clone(),
                source_url: url,
                content,
            });
        }

        if req.required {
            Err(AiDocsError::GitHubFileNotFound {
                repo: repo.to_string(),
                path: req.original_path.clone(),
                tried_tags: tried,
            })
        } else {
            Err(AiDocsError::OptionalFileNotFound(req.original_path.clone()))
        }
    }

    async fn send_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let first = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|source| AiDocsError::Fetch {
                url: url.to_string(),
                source,
            })?;

        if first.status() == StatusCode::TOO_MANY_REQUESTS
            || first.status() == StatusCode::FORBIDDEN
        {
            return Err(AiDocsError::Other(
                "GitHub API rate limit exceeded. Set GITHUB_TOKEN/GH_TOKEN.".to_string(),
            ));
        }

        if first.status().is_server_error() {
            debug!("GitHub 5xx for {url}; retrying once after 2s");
            sleep(Duration::from_secs(2)).await;
            let second =
                self.client
                    .get(url)
                    .send()
                    .await
                    .map_err(|source| AiDocsError::Fetch {
                        url: url.to_string(),
                        source,
                    })?;

            if second.status() == StatusCode::TOO_MANY_REQUESTS
                || second.status() == StatusCode::FORBIDDEN
            {
                return Err(AiDocsError::Other(
                    "GitHub API rate limit exceeded. Set GITHUB_TOKEN/GH_TOKEN.".to_string(),
                ));
            }

            return Ok(second);
        }

        Ok(first)
    }
}
