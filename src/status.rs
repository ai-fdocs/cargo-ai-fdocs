use std::collections::HashMap;
use std::path::Path;

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

impl DocsStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Synced => "Synced",
            Self::SyncedFallback => "SyncedFallback",
            Self::Outdated => "Outdated",
            Self::Missing => "Missing",
            Self::Corrupted => "Corrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateStatus {
    pub crate_name: String,
    pub lock_version: Option<String>,
    pub docs_version: Option<String>,
    pub status: DocsStatus,
}

#[derive(Debug, Deserialize)]
struct MetaFile {
    lock_version: Option<String>,
    version: Option<String>,
    is_fallback: Option<bool>,
    fallback: Option<bool>,
}

pub fn collect_status(
    config: &Config,
    lock_versions: &HashMap<String, String>,
    output_dir: &Path,
) -> Vec<CrateStatus> {
    let mut crate_names: Vec<_> = config.crates.keys().cloned().collect();
    crate_names.sort();

    crate_names
        .into_iter()
        .map(|crate_name| {
            let Some(lock_version) = lock_versions.get(&crate_name).cloned() else {
                return CrateStatus {
                    crate_name,
                    lock_version: None,
                    docs_version: None,
                    status: DocsStatus::Missing,
                };
            };

            let expected_dir = output_dir.join(format!("{crate_name}@{lock_version}"));

            if !expected_dir.is_dir() {
                let docs_version = discover_existing_version(output_dir, &crate_name);
                let status = if docs_version.is_some() {
                    DocsStatus::Outdated
                } else {
                    DocsStatus::Missing
                };

                return CrateStatus {
                    crate_name,
                    lock_version: Some(lock_version),
                    docs_version,
                    status,
                };
            }

            let meta_path = expected_dir.join(".aifd-meta.toml");
            let Ok(meta_raw) = std::fs::read_to_string(&meta_path) else {
                return CrateStatus {
                    crate_name,
                    lock_version: Some(lock_version.clone()),
                    docs_version: Some(lock_version),
                    status: DocsStatus::Corrupted,
                };
            };

            let Ok(meta) = toml::from_str::<MetaFile>(&meta_raw) else {
                return CrateStatus {
                    crate_name,
                    lock_version: Some(lock_version.clone()),
                    docs_version: Some(lock_version),
                    status: DocsStatus::Corrupted,
                };
            };

            let docs_version = meta
                .version
                .or(meta.lock_version)
                .unwrap_or(lock_version.clone());
            let status = if docs_version != lock_version {
                DocsStatus::Outdated
            } else if meta.is_fallback.or(meta.fallback).unwrap_or(false) {
                DocsStatus::SyncedFallback
            } else {
                DocsStatus::Synced
            };

            CrateStatus {
                crate_name,
                lock_version: Some(lock_version),
                docs_version: Some(docs_version),
                status,
            }
        })
        .collect()
}

fn discover_existing_version(output_dir: &Path, crate_name: &str) -> Option<String> {
    let mut versions = Vec::new();
    let prefix = format!("{crate_name}@");

    let Ok(entries) = std::fs::read_dir(output_dir) else {
        return None;
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }

        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();
        if let Some(version) = dir_name.strip_prefix(&prefix) {
            versions.push(version.to_string());
        }
    }

    versions.sort();
    versions.pop()
}

pub fn print_status_table(statuses: &[CrateStatus]) {
    let crate_col = statuses
        .iter()
        .map(|status| status.crate_name.len())
        .max()
        .unwrap_or(5)
        .max("crate".len());

    let lock_col = statuses
        .iter()
        .filter_map(|status| status.lock_version.as_ref().map(String::len))
        .max()
        .unwrap_or(4)
        .max("lock".len());

    let docs_col = statuses
        .iter()
        .filter_map(|status| status.docs_version.as_ref().map(String::len))
        .max()
        .unwrap_or(4)
        .max("docs".len());

    println!(
        "{:<crate_col$}  {:<lock_col$}  {:<docs_col$}  status",
        "crate", "lock", "docs"
    );
    println!(
        "{:-<crate_col$}  {:-<lock_col$}  {:-<docs_col$}  {:-<6}",
        "", "", "", ""
    );

    for status in statuses {
        println!(
            "{:<crate_col$}  {:<lock_col$}  {:<docs_col$}  {}",
            status.crate_name,
            status.lock_version.as_deref().unwrap_or("-"),
            status.docs_version.as_deref().unwrap_or("-"),
            status.status.as_str(),
        );
    }
}
