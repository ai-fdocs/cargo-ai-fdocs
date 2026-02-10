use reqwest::StatusCode;

use crate::error::{AiDocsError, Result};

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

pub struct GitHubFetcher {
    client: reqwest::Client,
}

impl GitHubFetcher {
    pub fn new() -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static("cargo-ai-fdocs/0.1"),
        );

        if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
            if let Ok(mut value) =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            {
                value.set_sensitive(true);
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("reqwest client");

        Self { client }
    }

    pub async fn resolve_ref(&self, repo: &str, version: &str) -> Result<ResolvedRef> {
        let repo_name = repo.rsplit('/').next().unwrap_or(repo);
        let candidates = vec![
            format!("v{version}"),
            version.to_string(),
            format!("{repo_name}-v{version}"),
            format!("{repo_name}-{version}"),
        ];

        for git_ref in candidates {
            let url = format!("https://raw.githubusercontent.com/{repo}/{git_ref}/README.md");
            let response =
                self.client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|source| AiDocsError::Fetch {
                        url: url.clone(),
                        source,
                    })?;

            if response.status().is_success() {
                return Ok(ResolvedRef {
                    git_ref,
                    is_fallback: false,
                });
            }
        }

        for fallback in ["main", "master"] {
            let url = format!("https://raw.githubusercontent.com/{repo}/{fallback}/README.md");
            let response =
                self.client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|source| AiDocsError::Fetch {
                        url: url.clone(),
                        source,
                    })?;
            if response.status().is_success() {
                return Ok(ResolvedRef {
                    git_ref: fallback.to_string(),
                    is_fallback: true,
                });
            }
        }

        Err(AiDocsError::Other(format!(
            "Could not resolve git ref for {repo}@{version}"
        )))
    }

    pub async fn fetch_files(
        &self,
        repo: &str,
        git_ref: &str,
        files: &[String],
    ) -> Vec<Result<FetchedFile>> {
        let file_list: Vec<String> = if files.is_empty() {
            vec!["README.md".to_string(), "CHANGELOG.md".to_string()]
        } else {
            files.to_vec()
        };

        let mut out = Vec::with_capacity(file_list.len());
        for path in file_list {
            out.push(self.fetch_file(repo, git_ref, &path).await);
        }
        out
    }

    async fn fetch_file(&self, repo: &str, git_ref: &str, path: &str) -> Result<FetchedFile> {
        let url = format!("https://raw.githubusercontent.com/{repo}/{git_ref}/{path}");
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|source| AiDocsError::Fetch {
                url: url.clone(),
                source,
            })?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(AiDocsError::GitHubFileNotFound {
                repo: repo.to_string(),
                path: path.to_string(),
                tried_tags: vec![git_ref.to_string()],
            });
        }

        if !response.status().is_success() {
            return Err(AiDocsError::Other(format!(
                "Unexpected status {} for {}",
                response.status(),
                url
            )));
        }

        let content = response.text().await.map_err(|source| AiDocsError::Fetch {
            url: url.clone(),
            source,
        })?;

        Ok(FetchedFile {
            path: path.to_string(),
            source_url: url,
            content,
        })
    }
}
