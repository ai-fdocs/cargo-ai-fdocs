use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;
use toml::Value;
use tracing::warn;

use crate::error::{AiDocsError, Result};

pub async fn run_init(config_path: &Path, force: bool) -> Result<()> {
    if config_path.exists() && !force {
        return Err(AiDocsError::Other(format!(
            "{} already exists. Use --force to overwrite",
            config_path.display()
        )));
    }

    let cargo_toml_path = Path::new("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(AiDocsError::Other("Cargo.toml not found".to_string()));
    }

    let content = std::fs::read_to_string(cargo_toml_path)?;
    let root: Value = toml::from_str(&content)?;

    let crate_names = collect_dependency_names(&root);
    if crate_names.is_empty() {
        return Err(AiDocsError::Other(
            "No dependencies found in Cargo.toml".to_string(),
        ));
    }

    let client = reqwest::Client::new();
    let mut resolved = BTreeMap::new();

    for crate_name in crate_names {
        match resolve_github_repo(&client, &crate_name).await {
            Ok(Some(repo)) => {
                resolved.insert(crate_name, repo);
            }
            Ok(None) => {
                warn!("Could not infer GitHub repo for crate '{crate_name}', skipping");
            }
            Err(e) => {
                warn!("Failed to resolve metadata for crate '{crate_name}': {e}");
            }
        }
    }

    if resolved.is_empty() {
        return Err(AiDocsError::Other(
            "Could not resolve any GitHub repositories from dependencies".to_string(),
        ));
    }

    let mut out = String::new();
    out.push_str("[settings]\n");
    out.push_str("output_dir = \"fdocs/rust\"\n");
    out.push_str("max_file_size_kb = 200\n");
    out.push_str("prune = true\n");
    out.push_str("docs_source = \"github\"\n\n");

    for (crate_name, repo) in resolved {
        out.push_str(&format!("[crates.{crate_name}]\n"));
        out.push_str(&format!("repo = \"{repo}\"\n\n"));
    }

    std::fs::write(config_path, out)?;
    Ok(())
}

fn collect_dependency_names(root: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    insert_table_keys(root.get("dependencies"), &mut names);
    insert_table_keys(
        root.get("workspace").and_then(|w| w.get("dependencies")),
        &mut names,
    );

    names
}

fn insert_table_keys(value: Option<&Value>, names: &mut BTreeSet<String>) {
    if let Some(table) = value.and_then(Value::as_table) {
        for name in table.keys() {
            names.insert(name.clone());
        }
    }
}

#[derive(Debug, Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_data: CrateData,
}

#[derive(Debug, Deserialize)]
struct CrateData {
    repository: Option<String>,
    homepage: Option<String>,
}

async fn resolve_github_repo(client: &reqwest::Client, crate_name: &str) -> Result<Option<String>> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}");
    let body: CratesIoResponse = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "cargo-ai-fdocs")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(body
        .crate_data
        .repository
        .or(body.crate_data.homepage)
        .and_then(|url| extract_github_owner_repo(&url)))
}

fn extract_github_owner_repo(url: &str) -> Option<String> {
    let normalized = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let marker = "github.com/";
    let idx = normalized.find(marker)?;
    let tail = &normalized[idx + marker.len()..];

    let mut parts = tail.split('/').filter(|p| !p.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?;

    Some(format!("{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::extract_github_owner_repo;

    #[test]
    fn extracts_repo_from_https_url() {
        assert_eq!(
            extract_github_owner_repo("https://github.com/tokio-rs/axum"),
            Some("tokio-rs/axum".to_string())
        );
    }

    #[test]
    fn extracts_repo_from_git_suffix() {
        assert_eq!(
            extract_github_owner_repo("https://github.com/serde-rs/serde.git"),
            Some("serde-rs/serde".to_string())
        );
    }
}
