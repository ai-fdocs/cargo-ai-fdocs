use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocsStatus {
    Synced,
    SyncedFallback,
    Outdated,
    Missing,
    Corrupted,
}

impl fmt::Display for DocsStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            DocsStatus::Synced => "Synced",
            DocsStatus::SyncedFallback => "SyncedFallback",
            DocsStatus::Outdated => "Outdated",
            DocsStatus::Missing => "Missing",
            DocsStatus::Corrupted => "Corrupted",
        };

        f.write_str(text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateStatus {
    pub name: String,
    pub lock_version: Option<String>,
    pub status: DocsStatus,
}

#[derive(Debug, Deserialize)]
struct MetaFile {
    #[serde(default)]
    is_fallback: bool,
    #[serde(default)]
    used_fallback: bool,
    #[serde(default)]
    fallback: bool,
}

pub fn collect_status(
    config: &Config,
    lock_versions: &HashMap<String, String>,
    output_dir: &Path,
) -> Vec<CrateStatus> {
    let mut statuses = Vec::new();

    for crate_name in config.crates.keys() {
        let status = match lock_versions.get(crate_name) {
            Some(version) => {
                let expected_dir = output_dir.join(format!("{crate_name}@{version}"));
                if expected_dir.is_dir() {
                    classify_expected_dir(&expected_dir)
                } else {
                    classify_unexpected_dirs(output_dir, crate_name)
                }
            }
            None => classify_unexpected_dirs(output_dir, crate_name),
        };

        statuses.push(CrateStatus {
            name: crate_name.to_string(),
            lock_version: lock_versions.get(crate_name).cloned(),
            status,
        });
    }

    statuses.sort_by(|a, b| a.name.cmp(&b.name));
    statuses
}

pub fn print_status_table(statuses: &[CrateStatus]) {
    println!("{:<24} {:<16} {}", "crate", "lock", "status");
    println!("{:-<24} {:-<16} {:-<10}", "", "", "");

    for item in statuses {
        let lock = item.lock_version.as_deref().unwrap_or("-");
        println!("{:<24} {:<16} {}", item.name, lock, item.status);
    }
}

fn classify_expected_dir(crate_dir: &Path) -> DocsStatus {
    let meta_path = crate_dir.join(".aifd-meta.toml");
    if !meta_path.is_file() {
        return DocsStatus::Corrupted;
    }

    match parse_meta_fallback_flag(&meta_path) {
        Some(true) => DocsStatus::SyncedFallback,
        Some(false) => DocsStatus::Synced,
        None => DocsStatus::Corrupted,
    }
}

fn classify_unexpected_dirs(output_dir: &Path, crate_name: &str) -> DocsStatus {
    let mut has_any_dir = false;

    for crate_dir in find_crate_dirs(output_dir, crate_name) {
        has_any_dir = true;

        let meta_path = crate_dir.join(".aifd-meta.toml");
        if !meta_path.is_file() || parse_meta_fallback_flag(&meta_path).is_none() {
            return DocsStatus::Corrupted;
        }
    }

    if has_any_dir {
        DocsStatus::Outdated
    } else {
        DocsStatus::Missing
    }
}

fn find_crate_dirs(output_dir: &Path, crate_name: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(output_dir) else {
        return Vec::new();
    };

    let prefix = format!("{crate_name}@");
    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }

            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if name.starts_with(&prefix) {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

fn parse_meta_fallback_flag(meta_path: &Path) -> Option<bool> {
    let content = std::fs::read_to_string(meta_path).ok()?;
    let meta: MetaFile = toml::from_str(&content).ok()?;

    Some(meta.is_fallback || meta.used_fallback || meta.fallback)
}
