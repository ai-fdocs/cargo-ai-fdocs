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
        let (markdown, truncated) = crate::storage::truncate_if_needed(&markdown, max_file_size_kb);
 
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
    let main_content = extract_main_content(crate_name, version, html);

    let mut out = String::new();
    out.push_str(&format!("# {crate_name}@{version}\n\n"));
    out.push_str("## Overview\n\n");
    out.push_str(&format!(
        "Generated from docs.rs page **{title}** for `{crate_name}` `{version}`.\n\n"
    ));

    if !main_content.is_empty() {
        out.push_str("## Documentation\n\n");
        out.push_str(&main_content);
        out.push_str("\n\n");
    }

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

fn extract_main_content(crate_name: &str, version: &str, html: &str) -> String {
    // docs.rs usually has the main content in <div id="main-content"> or <div class="docblock">
    let mut content = String::new();

    if let Some(start) = html.find("<div id=\"main-content\"") {
        let rest = &html[start..];
        if let Some(end) = find_closing_div(rest) {
            content = strip_html_tags(crate_name, version, &rest[..end]);
        }
    } else if let Some(start) = html.find("<div class=\"docblock\"") {
        let rest = &html[start..];
        if let Some(end) = find_closing_div(rest) {
            content = strip_html_tags(crate_name, version, &rest[..end]);
        }
    }

    content.trim().to_string()
}

fn find_closing_div(html: &str) -> Option<usize> {
    let mut depth = 0;
    let mut i = 0;
    let bytes = html.as_bytes();
    
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<div") {
            // Check if it's a real div tag start (followed by space or >)
            let next_char = bytes.get(i + 4);
            if next_char.is_none() || matches!(next_char, Some(b' ') | Some(b'>')) {
                depth += 1;
                i += 4;
                continue;
            }
        }
        if bytes[i..].starts_with(b"</div>") {
            depth -= 1;
            i += 6;
            if depth == 0 {
                return Some(i);
            }
            continue;
        }
        i += 1;
    }
    None
}

fn strip_html_tags(crate_name: &str, version: &str, html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut in_pre = false;
    let mut tag_buffer = String::new();

    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<pre") {
            in_pre = true;
            out.push_str("\n```rust\n");
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() { i += 1; }
            continue;
        }
        if bytes[i..].starts_with(b"</pre>") {
            in_pre = false;
            out.push_str("\n```\n");
            i += 6;
            continue;
        }

        if bytes[i] == b'<' {
            in_tag = true;
            tag_buffer.clear();
        } else if bytes[i] == b'>' {
            in_tag = false;
            
            // Handle basic link rewriting if we just closed an <a> tag
            let normalized_tag = tag_buffer.to_lowercase();
            if normalized_tag.starts_with("a ") {
                if let Some(abs_href) = extract_href(&tag_buffer) {
                    // We record the link to be appended after the link text
                    out.push_str(" (");
                    out.push_str(&abs_href);
                    out.push(')');
                }
            }
            
            // Add spacing for structural tags
            if normalized_tag.starts_with("p") || normalized_tag.starts_with("/p") || 
               normalized_tag.starts_with("h") || normalized_tag.starts_with("/h") ||
               normalized_tag.starts_with("li") || normalized_tag.starts_with("/li") ||
               normalized_tag.starts_with("div") || normalized_tag.starts_with("/div") ||
               normalized_tag.starts_with("br") {
                out.push('\n');
            }
        } else if in_tag {
            tag_buffer.push(bytes[i] as char);
        } else if !in_tag {
            // Handle common entities
            if bytes[i..].starts_with(b"&nbsp;") {
                out.push(' ');
                i += 5;
            } else if bytes[i..].starts_with(b"&lt;") {
                out.push('<');
                i += 3;
            } else if bytes[i..].starts_with(b"&gt;") {
                out.push('>');
                i += 3;
            } else if bytes[i..].starts_with(b"&amp;") {
                out.push('&');
                i += 4;
            } else if bytes[i..].starts_with(b"&quot;") {
                out.push('"');
                i += 5;
            } else {
                out.push(bytes[i] as char);
            }
        }
        i += 1;
    }

    // Unescape some basic entities and clean up whitespace
    let result = out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    
    clean_markdown_whitespace(&result)
}

fn extract_href(tag: &str) -> Option<String> {
    let tag_lower = tag.to_lowercase();
    let href_start = tag_lower.find("href=")?;
    let mut val_part = &tag[href_start + 5..].trim_start();
    
    if val_part.starts_with('"') || val_part.starts_with('\'') {
        let quote = val_part.chars().next().unwrap();
        val_part = &val_part[1..];
        if let Some(end) = val_part.find(quote) {
            let href = &val_part[..end];
            return Some(_href_to_absolute(href));
        }
    } else {
        // Unquoted (less common but possible in messy HTML)
        let end = val_part.find(|c| matches!(c, ' ' | '>')).unwrap_or(val_part.len());
        let href = &val_part[..end];
        return Some(_href_to_absolute(href));
    }
    None
}

fn _href_to_absolute(href: &str) -> String {
    if href.starts_with("http") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("https://docs.rs{}", href)
    } else {
        href.to_string()
    }
}

fn clean_markdown_whitespace(s: &str) -> String {
    let mut out = String::new();
    let mut last_was_empty = false;
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !last_was_empty {
                out.push_str("\n");
                last_was_empty = true;
            }
        } else {
            out.push_str(trimmed);
            out.push('\n');
            last_was_empty = false;
        }
    }
    out.trim().to_string()
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

#[cfg(test)]
mod tests {
    use super::{
        extract_docs_links, extract_title, is_docsrs_fallback_eligible,
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
        let (truncated, is_truncated) = crate::storage::truncate_if_needed(&content, 1);
        assert!(is_truncated);
        assert!(truncated.contains("[TRUNCATED by ai-fdocs at 1KB]"));
    }

    #[test]
    fn test_extract_main_content_simple() {
        let html = r#"<div id="main-content"><h1>Hello</h1><p>World</p></div>"#;
        assert_eq!(super::extract_main_content("test", "0.1.0", html), "Hello\nWorld");
    }

    #[test]
    fn test_extract_main_content_with_code() {
        let html = r#"<div class="docblock"><pre>pub fn test() {}</pre></div>"#;
        let content = super::extract_main_content("test", "0.1.0", html);
        assert!(content.contains("```rust"));
        assert!(content.contains("pub fn test() {}"));
        assert!(content.contains("```"));
    }

    #[test]
    fn test_strip_html_tags_with_links_and_spacing() {
        let html = r#"<div class="docblock"><h1>Title</h1><p>Para with <a href="/serde/1.0.0/serde/index.html">link</a>.</p><ul><li>Item 1</li><li>Item 2</li></ul></div>"#;
        let content = super::strip_html_tags("serde", "1.0.0", html);
        // Note: Our current rudimentary implementation doesn't do full link rewriting yet, 
        // but it should at least handle the spacing and tag removal.
        assert!(content.contains("Title"));
        assert!(content.contains("Para with link."));
        assert!(content.contains("Item 1"));
        assert!(content.contains("Item 2"));
    }

    #[test]
    fn test_clean_markdown_whitespace() {
        let input = "Line 1\n\n\nLine 2\n   \nLine 3\n";
        let expected = "Line 1\n\nLine 2\n\nLine 3";
        assert_eq!(super::clean_markdown_whitespace(input), expected);
    }
}
