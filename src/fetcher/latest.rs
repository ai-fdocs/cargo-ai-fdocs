use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::debug;

use crate::error::{AiDocsError, Result};

const APP_USER_AGENT: &str = concat!("cargo-ai-fdocs/", env!("CARGO_PKG_VERSION"));
const MAX_RETRY_ATTEMPTS: usize = 3;
const RETRY_BASE_BACKOFF_MS: u64 = 500;

pub struct LatestDocsFetcher {
    client: Client,
}

#[derive(Debug, Clone)]
pub struct DocsRsArtifact {
    pub markdown: String,
    pub docsrs_input_url: String,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_data: CratesIoCrate,
}

#[derive(Debug, Deserialize)]
struct CratesIoCrate {
    max_stable_version: Option<String>,
    max_version: Option<String>,
}

impl LatestDocsFetcher {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        Self { client }
    }

    pub async fn resolve_latest_version(&self, crate_name: &str) -> Result<String> {
        let url = format!("https://crates.io/api/v1/crates/{crate_name}");
        let response = self.send_with_retry(&url).await?;
        if !response.status().is_success() {
            return Err(AiDocsError::HttpStatus {
                url,
                status: response.status().as_u16(),
            });
        }

        let body: CratesIoResponse = response.json().await?;
        body.crate_data
            .max_stable_version
            .filter(|v| !v.trim().is_empty())
            .or(body.crate_data.max_version)
            .ok_or_else(|| {
                AiDocsError::Other(format!(
                    "crates.io response for '{crate_name}' has no max version"
                ))
            })
    }

    pub async fn fetch_api_markdown(
        &self,
        crate_name: &str,
        version: &str,
        max_file_size_kb: usize,
    ) -> Result<DocsRsArtifact> {
        let docsrs_input_url = format!("https://docs.rs/crate/{crate_name}/{version}");
        let response = self.send_with_retry(&docsrs_input_url).await?;
        if !response.status().is_success() {
            return Err(AiDocsError::HttpStatus {
                url: docsrs_input_url,
                status: response.status().as_u16(),
            });
        }

        let html = response.text().await?;
        let markdown = render_docsrs_markdown(crate_name, version, &html);
        let (markdown, truncated) = truncate_markdown(&markdown, max_file_size_kb);

        Ok(DocsRsArtifact {
            markdown,
            docsrs_input_url: format!("https://docs.rs/crate/{crate_name}/{version}"),
            truncated,
        })
    }

    async fn send_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut backoff_ms = RETRY_BASE_BACKOFF_MS;

        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            match self.client.get(url).send().await {
                Ok(response) => {
                    let status = response.status();
                    let retryable_status =
                        status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();

                    if retryable_status && attempt < MAX_RETRY_ATTEMPTS {
                        debug!(
                            "latest-docs upstream {status} for {url}; retrying attempt {}/{} after {}ms",
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
                    let retryable_network =
                        source.is_timeout() || source.is_connect() || source.is_request();

                    if retryable_network && attempt < MAX_RETRY_ATTEMPTS {
                        debug!(
                            "latest-docs network error for {url}; retrying attempt {}/{} after {}ms: {source}",
                            attempt + 1,
                            MAX_RETRY_ATTEMPTS,
                            backoff_ms
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;
                        continue;
                    }

                    return Err(AiDocsError::Http(source));
                }
            }
        }

        Err(AiDocsError::Other(
            "unexpected retry flow termination".to_string(),
        ))
    }
}

pub fn is_docsrs_fallback_eligible(error: &AiDocsError) -> bool {
    match error {
        AiDocsError::HttpStatus { status, .. } => {
            *status == StatusCode::NOT_FOUND.as_u16()
                || *status == StatusCode::TOO_MANY_REQUESTS.as_u16()
                || (*status >= 500 && *status < 600)
        }
        AiDocsError::Http(_) | AiDocsError::Fetch { .. } => true,
        _ => false,
    }
}

fn render_docsrs_markdown(crate_name: &str, version: &str, html: &str) -> String {
    let canonical_base = format!("https://docs.rs/{crate_name}/{version}");
    let input_url = format!("https://docs.rs/crate/{crate_name}/{version}");
    let title = extract_title(html).unwrap_or_else(|| format!("{crate_name} {version}"));
    let links = extract_docs_links(crate_name, version, html);

    let mut out = String::new();
    out.push_str(&format!("# {crate_name}@{version}\n\n"));
    out.push_str("## Overview\n\n");
    out.push_str(&format!(
        "Generated from docs.rs page **{title}** for `{crate_name}` `{version}`.\n\n"
    ));

    out.push_str("## API Reference\n\n");
    out.push_str(&format!("- [crate page]({input_url})\n"));
    out.push_str(&format!(
        "- [rustdoc root]({canonical_base}/{crate_name}/)\n"
    ));
    for link in links.into_iter().take(20) {
        out.push_str(&format!("- [{link}](https://docs.rs{link})\n"));
    }

    out.push_str("\n## Example\n\n");
    out.push_str("```rust\n");
    out.push_str(&format!("use {crate_name} as _;\n"));
    out.push_str("```\n\n");

    out.push_str("---\n");
    out.push_str(&format!("Source: {input_url}\n"));

    out
}

fn extract_title(html: &str) -> Option<String> {
    let start = html.find("<title>")? + "<title>".len();
    let end = html[start..].find("</title>")? + start;
    Some(html[start..end].trim().to_string())
}

fn extract_docs_links(crate_name: &str, version: &str, html: &str) -> Vec<String> {
    let needle = format!("href=\"/{crate_name}/{version}/");
    let mut links = Vec::new();
    let mut idx = 0;
    while let Some(found) = html[idx..].find(&needle) {
        let start = idx + found + "href=\"".len();
        let rest = &html[start..];
        let Some(end) = rest.find('"') else {
            break;
        };
        let href = &rest[..end];
        if !links.iter().any(|v| v == href) {
            links.push(href.to_string());
        }
        idx = start + end;
    }
    links
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn truncate_markdown(content: &str, max_size_kb: usize) -> (String, bool) {
    let max_bytes = max_size_kb * 1024;
    if content.len() <= max_bytes {
        return (content.to_string(), false);
    }
    let boundary = floor_char_boundary(content, max_bytes);
    let mut out = content[..boundary].to_string();
    out.push_str(&format!("\n\n[TRUNCATED by ai-fdocs at {max_size_kb}KB]\n"));
    (out, true)
}

#[cfg(test)]
mod tests {
    use super::{
        extract_docs_links, extract_title, is_docsrs_fallback_eligible, truncate_markdown,
    };
    use crate::error::AiDocsError;

    #[test]
    fn extracts_title() {
        let html = "<html><head><title>serde - Rust</title></head></html>";
        assert_eq!(extract_title(html).as_deref(), Some("serde - Rust"));
    }

    #[test]
    fn extracts_unique_docs_links() {
        let html = r#"<a href="/serde/1.0.0/serde/">A</a><a href="/serde/1.0.0/serde/">B</a>"#;
        let links = extract_docs_links("serde", "1.0.0", html);
        assert_eq!(links, vec!["/serde/1.0.0/serde/"]);
    }

    #[test]
    fn marks_fallback_eligible_statuses() {
        assert!(is_docsrs_fallback_eligible(&AiDocsError::HttpStatus {
            url: "u".to_string(),
            status: 404,
        }));
        assert!(is_docsrs_fallback_eligible(&AiDocsError::HttpStatus {
            url: "u".to_string(),
            status: 429,
        }));
        assert!(is_docsrs_fallback_eligible(&AiDocsError::HttpStatus {
            url: "u".to_string(),
            status: 503,
        }));
        assert!(!is_docsrs_fallback_eligible(&AiDocsError::HttpStatus {
            url: "u".to_string(),
            status: 401,
        }));
    }

    #[test]
    fn truncates_when_limit_exceeded() {
        let content = "x".repeat(5000);
        let (truncated, is_truncated) = truncate_markdown(&content, 1);
        assert!(is_truncated);
        assert!(truncated.contains("[TRUNCATED by ai-fdocs at 1KB]"));
    }
}
