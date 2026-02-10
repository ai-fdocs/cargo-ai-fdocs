use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use tracing::{debug, info};

use crate::config::{Config, CrateDoc};
use crate::error::{AiDocsError, Result};
use crate::fetcher::github::{FetchedFile, ResolvedRef};
use crate::processor::changelog;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct CrateMeta {
    pub version: String,
    pub git_ref: String,
    pub fetched_at: String,
    pub is_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct SavedCrate {
    pub name: String,
    pub version: String,
    pub git_ref: String,
    pub is_fallback: bool,
    pub files: Vec<String>,
    pub ai_notes: String,
}

pub fn flatten_filename(file_path: &str) -> String {
    if file_path.contains('/') {
        file_path.replace('/', "__")
    } else {
        file_path.to_string()
    }
}

fn inject_header(
    content: &str,
    owner_repo: &str,
    git_ref: &str,
    original_path: &str,
    is_fallback: bool,
    version: &str,
    source_url: &str,
) -> String {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let mut header = format!(
        "<!-- AI-FDOCS: source=github.com/{owner_repo} ref={git_ref} path={original_path} fetched={date} -->\n<!-- AI-FDOCS: url={source_url} -->\n"
    );

    if is_fallback {
        header.push_str(&format!(
            "<!-- AI-FDOCS WARNING: No tag found for version {version}. Fetched from '{git_ref}' branch. Content may not match installed version. -->\n"
        ));
    }

    format!("{header}\n{content}")
}

fn should_inject_header(file_path: &str) -> bool {
    let lower = file_path.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".html") || lower.ends_with(".htm")
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn truncate_if_needed(content: &str, max_size_kb: usize) -> String {
    let max_bytes = max_size_kb * 1024;
    if content.len() <= max_bytes {
        return content.to_string();
    }

    let boundary = floor_char_boundary(content, max_bytes);
    let truncated = &content[..boundary];
    format!("{truncated}\n\n[TRUNCATED by ai-fdocs at {max_size_kb}KB]\n")
}

pub fn is_cached(output_dir: &Path, crate_name: &str, version: &str) -> bool {
    let crate_dir = output_dir.join(format!("{crate_name}@{version}"));
    let meta_path = crate_dir.join(".aifd-meta.toml");

    if !meta_path.exists() {
        return false;
    }

    match fs::read_to_string(&meta_path) {
        Ok(content) => match toml::from_str::<CrateMeta>(&content) {
            Ok(meta) => meta.version == version,
            Err(_) => false,
        },
        Err(_) => false,
    }
}

pub fn save_crate_files(
    output_dir: &Path,
    crate_name: &str,
    version: &str,
    repo: &str,
    resolved: &ResolvedRef,
    fetched_files: &[FetchedFile],
    crate_config: &CrateDoc,
    max_file_size_kb: usize,
) -> Result<SavedCrate> {
    let crate_dir = output_dir.join(format!("{crate_name}@{version}"));

    if crate_dir.exists() {
        fs::remove_dir_all(&crate_dir)?;
    }
    fs::create_dir_all(&crate_dir)?;

    let mut saved_names = Vec::new();

    for file in fetched_files {
        let flat_name = flatten_filename(&file.path);
        let mut content = file.content.clone();

        if file.path.to_lowercase().contains("changelog") {
            content = changelog::truncate_changelog(&content, version);
        }

        content = truncate_if_needed(&content, max_file_size_kb);

        if should_inject_header(&file.path) {
            content = inject_header(
                &content,
                repo,
                &resolved.git_ref,
                &file.path,
                resolved.is_fallback,
                version,
                &file.source_url,
            );
        }

        let file_path = crate_dir.join(&flat_name);
        fs::write(&file_path, &content)?;
        debug!("Saved: {:?}", file_path);
        saved_names.push(flat_name);
    }

    let meta = CrateMeta {
        version: version.to_string(),
        git_ref: resolved.git_ref.clone(),
        fetched_at: Utc::now().format("%Y-%m-%d").to_string(),
        is_fallback: resolved.is_fallback,
    };

    let meta_content = toml::to_string_pretty(&meta)
        .map_err(|e| AiDocsError::Other(format!("Failed to serialize meta: {e}")))?;
    fs::write(crate_dir.join(".aifd-meta.toml"), meta_content)?;

    info!(
        "  💾 {}@{}: {} files saved to {:?}",
        crate_name,
        version,
        saved_names.len(),
        crate_dir
    );

    Ok(SavedCrate {
        name: crate_name.to_string(),
        version: version.to_string(),
        git_ref: resolved.git_ref.clone(),
        is_fallback: resolved.is_fallback,
        files: saved_names,
        ai_notes: crate_config.ai_notes.clone(),
    })
}

pub fn read_cached_info(
    output_dir: &Path,
    crate_name: &str,
    version: &str,
    crate_config: &CrateDoc,
) -> Option<SavedCrate> {
    let crate_dir = output_dir.join(format!("{crate_name}@{version}"));
    let meta_path = crate_dir.join(".aifd-meta.toml");
    let meta_str = fs::read_to_string(&meta_path).ok()?;
    let meta: CrateMeta = toml::from_str(&meta_str).ok()?;

    let files: Vec<String> = fs::read_dir(&crate_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            if name.starts_with('.') {
                None
            } else {
                Some(name)
            }
        })
        .collect();

    Some(SavedCrate {
        name: crate_name.to_string(),
        version: version.to_string(),
        git_ref: meta.git_ref,
        is_fallback: meta.is_fallback,
        files,
        ai_notes: crate_config.ai_notes.clone(),
    })
}

pub fn prune(
    output_dir: &Path,
    config: &Config,
    lock_versions: &HashMap<String, String>,
) -> Result<()> {
    if !output_dir.exists() {
        return Ok(());
    }

    let configured: HashSet<&str> = config.crates.keys().map(String::as_str).collect();

    for entry in fs::read_dir(output_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let Some((crate_name, dir_version)) = split_name_version(dir_name) else {
            continue;
        };

        let should_remove = if !configured.contains(crate_name) {
            true
        } else {
            match lock_versions.get(crate_name) {
                Some(lock_ver) => lock_ver != dir_version,
                None => true,
            }
        };

        if should_remove {
            info!("  🗑 Pruning {dir_name}");
            fs::remove_dir_all(path)?;
        }
    }

    Ok(())
}

fn split_name_version(dir_name: &str) -> Option<(&str, &str)> {
    let (name, version) = dir_name.rsplit_once('@')?;
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some((name, version))
}

pub fn rust_output_dir(base_output_dir: &Path) -> PathBuf {
    if base_output_dir.file_name().and_then(|n| n.to_str()) == Some("rust") {
        return base_output_dir.to_path_buf();
    }
    base_output_dir.join("rust")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_root_file() {
        assert_eq!(flatten_filename("README.md"), "README.md");
    }

    #[test]
    fn test_flatten_nested_file() {
        assert_eq!(
            flatten_filename("docs/guides/overview.md"),
            "docs__guides__overview.md"
        );
    }

    #[test]
    fn test_should_inject_header() {
        assert!(should_inject_header("README.md"));
        assert!(should_inject_header("guide.html"));
        assert!(!should_inject_header("example.rs"));
    }

    #[test]
    fn test_truncate_large_file() {
        let content = "x".repeat(300 * 1024);
        let result = truncate_if_needed(&content, 200);
        assert!(result.contains("[TRUNCATED by ai-fdocs at 200KB]"));
    }

    #[test]
    fn test_split_name_version() {
        assert_eq!(split_name_version("serde@1.0.0"), Some(("serde", "1.0.0")));
        assert_eq!(split_name_version("serde"), None);
    }
}
