use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
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
    print!("{}", format_status_table(statuses));
}

fn format_status_table(statuses: &[CrateStatus]) -> String {
    const COL_CRATE: usize = 24;
    const COL_LOCK: usize = 16;
    const COL_STATUS: usize = 14;

    let mut output = String::new();
    let _ = writeln!(
        output,
        "{:<COL_CRATE$} {:<COL_LOCK$} {:<COL_STATUS$}",
        "Crate", "Lock Version", "Docs Status"
    );
    let _ = writeln!(
        output,
        "{:-<COL_CRATE$} {:-<COL_LOCK$} {:-<COL_STATUS$}",
        "", "", ""
    );

    for item in statuses {
        let lock = item.lock_version.as_deref().unwrap_or("-");
        let _ = writeln!(
            output,
            "{:<COL_CRATE$} {:<COL_LOCK$} {:<COL_STATUS$}",
            item.name, lock, item.status
        );
    }

    let summary = summarize(statuses);
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "Total: {} | Synced: {} | Missing: {} | Outdated: {} | Corrupted: {}",
        summary.total, summary.synced, summary.missing, summary.outdated, summary.corrupted
    );

    if summary.has_problems() {
        let _ = writeln!(output, "Hint: cargo ai-docs sync --force");
    }

    output
}

#[derive(Debug, Default)]
struct StatusSummary {
    total: usize,
    synced: usize,
    missing: usize,
    outdated: usize,
    corrupted: usize,
}

impl StatusSummary {
    fn has_problems(&self) -> bool {
        self.missing > 0 || self.outdated > 0 || self.corrupted > 0
    }
}

fn summarize(statuses: &[CrateStatus]) -> StatusSummary {
    let mut summary = StatusSummary {
        total: statuses.len(),
        ..StatusSummary::default()
    };

    for item in statuses {
        match item.status {
            DocsStatus::Synced | DocsStatus::SyncedFallback => summary.synced += 1,
            DocsStatus::Missing => summary.missing += 1,
            DocsStatus::Outdated => summary.outdated += 1,
            DocsStatus::Corrupted => summary.corrupted += 1,
        }
    }

    summary
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

#[cfg(test)]
mod tests {
    use super::{format_status_table, CrateStatus, DocsStatus};

    #[test]
    fn formats_empty_status_table_with_zero_summary() {
        let table = format_status_table(&[]);

        assert!(table.contains("Crate"));
        assert!(table.contains("Lock Version"));
        assert!(table.contains("Docs Status"));
        assert!(table.contains("Total: 0 | Synced: 0 | Missing: 0 | Outdated: 0 | Corrupted: 0"));
        assert!(!table.contains("Hint: cargo ai-docs sync --force"));
    }

    #[test]
    fn formats_missing_lock_version_and_shows_hint_for_problems() {
        let statuses = vec![CrateStatus {
            name: "serde".to_string(),
            lock_version: None,
            status: DocsStatus::Missing,
        }];

        let table = format_status_table(&statuses);

        assert!(table.contains("serde"));
        assert!(table.contains("-"));
        assert!(table.contains("Missing"));
        assert!(table.contains("Total: 1 | Synced: 0 | Missing: 1 | Outdated: 0 | Corrupted: 0"));
        assert!(table.contains("Hint: cargo ai-docs sync --force"));
    }
}
