use std::env;
use std::time::Duration;

const MAX_RETRY_ATTEMPTS: usize = 3;
const RETRY_BASE_BACKOFF_MS: u64 = 500;

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
    api_base_url: String,
    raw_base_url: String,
}

#[derive(Deserialize)]
struct RepoInfo {
    default_branch: String,
}

impl GitHubFetcher {
    pub fn new() -> Self {
        Self::with_base_urls_internal(
            "https://api.github.com",
            "https://raw.githubusercontent.com",
            false,
        )
    }

    #[cfg(test)]
    fn with_base_urls_no_proxy(api_base_url: &str, raw_base_url: &str) -> Self {
        Self::with_base_urls_internal(api_base_url, raw_base_url, true)
    }

    fn with_base_urls_internal(api_base_url: &str, raw_base_url: &str, no_proxy: bool) -> Self {
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

        let mut builder = Client::builder()
            .user_agent(APP_USER_AGENT)
            .default_headers(headers)
            .timeout(Duration::from_secs(30));

        if no_proxy {
            builder = builder.no_proxy();
        }

        let client = builder.build().expect("reqwest client");

        Self {
            client,
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            raw_base_url: raw_base_url.trim_end_matches('/').to_string(),
        }
    }

    fn api_tag_url(&self, owner_repo: &str, tag: &str) -> String {
        format!(
            "{}/repos/{owner_repo}/git/ref/tags/{tag}",
            self.api_base_url
        )
    }

    fn api_repo_url(&self, owner_repo: &str) -> String {
        format!("{}/repos/{owner_repo}", self.api_base_url)
    }

    fn raw_file_url(&self, repo: &str, git_ref: &str, candidate: &str) -> String {
        format!("{}/{repo}/{git_ref}/{candidate}", self.raw_base_url)
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
            let url = self.api_tag_url(owner_repo, &tag);
            let res = self.send_with_retry(url.as_str()).await?;
            if res.status().is_success() {
                return Ok(ResolvedRef {
                    git_ref: tag,
                    is_fallback: false,
                });
            }

            if res.status() != StatusCode::NOT_FOUND {
                return Err(Self::status_error(url.as_str(), res.status()));
            }
        }

        let repo_url = self.api_repo_url(owner_repo);
        let repo_resp = self.send_with_retry(repo_url.as_str()).await?;
        if !repo_resp.status().is_success() {
            return Err(Self::status_error(repo_url.as_str(), repo_resp.status()));
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
            let url = self.raw_file_url(repo, git_ref, candidate);
            let res = self.send_with_retry(url.as_str()).await?;

            if res.status() == StatusCode::NOT_FOUND {
                continue;
            }

            if !res.status().is_success() {
                return Err(Self::status_error(url.as_str(), res.status()));
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
        let mut backoff_ms = RETRY_BASE_BACKOFF_MS;

        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            let send_result = self.client.get(url).send().await;

            match send_result {
                Ok(response) => {
                    let status = response.status();

                    if status == StatusCode::UNAUTHORIZED {
                        return Err(AiDocsError::GitHubAuth {
                            url: url.to_string(),
                            status: status.as_u16(),
                        });
                    }

                    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
                        return Err(AiDocsError::GitHubRateLimit {
                            url: url.to_string(),
                            status: status.as_u16(),
                        });
                    }

                    if status.is_server_error() && attempt < MAX_RETRY_ATTEMPTS {
                        debug!(
                            "GitHub {status} for {url}; retrying attempt {}/{} after {}ms",
                            attempt + 1,
                            MAX_RETRY_ATTEMPTS,
                            backoff_ms
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;
                        continue;
                    }

                    return Ok(response);
                }
                Err(source) => {
                    let is_retryable_network =
                        source.is_timeout() || source.is_connect() || source.is_request();

                    if is_retryable_network && attempt < MAX_RETRY_ATTEMPTS {
                        debug!(
                            "Network error for {url}; retrying attempt {}/{} after {}ms: {source}",
                            attempt + 1,
                            MAX_RETRY_ATTEMPTS,
                            backoff_ms
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;
                        continue;
                    }

                    return Err(AiDocsError::Fetch {
                        url: url.to_string(),
                        source,
                    });
                }
            }
        }

        Err(AiDocsError::Other(
            "unexpected retry flow termination".to_string(),
        ))
    }

    fn status_error(url: &str, status: StatusCode) -> AiDocsError {
        match status {
            StatusCode::UNAUTHORIZED => AiDocsError::GitHubAuth {
                url: url.to_string(),
                status: status.as_u16(),
            },
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => AiDocsError::GitHubRateLimit {
                url: url.to_string(),
                status: status.as_u16(),
            },
            _ => AiDocsError::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn start_mock_server(routes: HashMap<String, (u16, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        let routes = Arc::new(Mutex::new(routes));

        thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let mut buf = [0_u8; 4096];
                let read = match stream.read(&mut buf) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                if read == 0 {
                    continue;
                }

                let req = String::from_utf8_lossy(&buf[..read]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");

                let (status, body) = routes
                    .lock()
                    .expect("lock routes")
                    .get(path)
                    .cloned()
                    .unwrap_or((404, String::new()));

                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        format!("http://{addr}")
    }

    #[tokio::test]
    async fn resolves_fallback_to_default_branch_when_tags_missing() {
        let mut routes = HashMap::new();
        routes.insert(
            "/repos/owner/repo/git/ref/tags/v1.2.3".to_string(),
            (404, String::new()),
        );
        routes.insert(
            "/repos/owner/repo/git/ref/tags/1.2.3".to_string(),
            (404, String::new()),
        );
        routes.insert(
            "/repos/owner/repo/git/ref/tags/demo-v1.2.3".to_string(),
            (404, String::new()),
        );
        routes.insert(
            "/repos/owner/repo/git/ref/tags/demo-1.2.3".to_string(),
            (404, String::new()),
        );
        routes.insert(
            "/repos/owner/repo".to_string(),
            (200, "{\"default_branch\":\"main\"}".to_string()),
        );

        let api_base = start_mock_server(routes);
        let fetcher =
            GitHubFetcher::with_base_urls_no_proxy(api_base.as_str(), "http://raw.invalid");

        let resolved = fetcher
            .resolve_ref("owner/repo", "demo", "1.2.3")
            .await
            .expect("resolve fallback ref");
        assert_eq!(resolved.git_ref, "main");
        assert!(resolved.is_fallback);
    }

    #[tokio::test]
    async fn fetch_files_reports_partial_failures_and_optional_miss() {
        let mut routes = HashMap::new();
        routes.insert(
            "/owner/repo/main/README.md".to_string(),
            (200, "doc".to_string()),
        );
        routes.insert(
            "/owner/repo/main/CHANGELOG.md".to_string(),
            (404, String::new()),
        );
        routes.insert("/owner/repo/main/LICENSE".to_string(), (404, String::new()));

        let raw_base = start_mock_server(routes);
        let fetcher =
            GitHubFetcher::with_base_urls_no_proxy("http://api.invalid", raw_base.as_str());

        let requests = vec![
            FileRequest {
                original_path: "README.md".to_string(),
                candidates: vec!["README.md".to_string()],
                required: true,
            },
            FileRequest {
                original_path: "CHANGELOG.md".to_string(),
                candidates: vec!["CHANGELOG.md".to_string()],
                required: true,
            },
            FileRequest {
                original_path: "LICENSE".to_string(),
                candidates: vec!["LICENSE".to_string()],
                required: false,
            },
        ];

        let results = fetcher.fetch_files("owner/repo", "main", &requests).await;
        assert_eq!(results.len(), 3);

        assert!(results[0].is_ok());
        assert!(matches!(
            &results[1],
            Err(AiDocsError::GitHubFileNotFound { .. })
        ));
        assert!(matches!(
            &results[2],
            Err(AiDocsError::OptionalFileNotFound(path)) if path == "LICENSE"
        ));
    }
}
