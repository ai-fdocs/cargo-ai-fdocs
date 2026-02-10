use std::collections::HashMap;
use std::fmt::Write as _;
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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synced => "Synced",
            Self::SyncedFallback => "SyncedFallback",
            Self::Outdated => "Outdated",
            Self::Missing => "Missing",
            Self::Corrupted => "Corrupted",
        }
    }

    fn is_problem(self) -> bool {
        matches!(self, Self::Outdated | Self::Missing | Self::Corrupted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateStatus {
    pub crate_name: String,
    pub lock_version: Option<String>,
    pub docs_version: Option<String>,
    pub status: DocsStatus,
    pub reason: String,
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
                    reason: "crate missing in Cargo.lock".to_string(),
                };
            };

            let expected_dir = output_dir.join(format!("{crate_name}@{lock_version}"));
            if !expected_dir.is_dir() {
                let docs_version = discover_existing_version(output_dir, &crate_name);
                let (status, reason) = if let Some(existing) = docs_version.clone() {
                    (
                        DocsStatus::Outdated,
                        format!(
                            "cached docs version {existing} differs from lock version {lock_version}"
                        ),
                    )
                } else {
                    (
                        DocsStatus::Missing,
                        "no synced docs found for this crate".to_string(),
                    )
                };

                return CrateStatus {
                    crate_name,
                    lock_version: Some(lock_version),
                    docs_version,
                    status,
                    reason,
                };
            }

            let meta_path = expected_dir.join(".aifd-meta.toml");
            let Ok(meta_raw) = std::fs::read_to_string(&meta_path) else {
                return CrateStatus {
                    crate_name,
                    lock_version: Some(lock_version.clone()),
                    docs_version: Some(lock_version),
                    status: DocsStatus::Corrupted,
                    reason: ".aifd-meta.toml is missing or unreadable".to_string(),
                };
            };

            let Ok(meta) = toml::from_str::<MetaFile>(&meta_raw) else {
                return CrateStatus {
                    crate_name,
                    lock_version: Some(lock_version.clone()),
                    docs_version: Some(lock_version),
                    status: DocsStatus::Corrupted,
                    reason: ".aifd-meta.toml has invalid TOML".to_string(),
                };
            };

            let docs_version = meta
                .version
                .or(meta.lock_version)
                .unwrap_or_else(|| lock_version.clone());

            let (status, reason) = if docs_version != lock_version {
                (
                    DocsStatus::Outdated,
                    format!(
                        "metadata version {docs_version} differs from lock version {lock_version}"
                    ),
                )
            } else if meta.is_fallback.or(meta.fallback).unwrap_or(false) {
                (
                    DocsStatus::SyncedFallback,
                    "synced from fallback branch (no exact tag found)".to_string(),
                )
            } else {
                (DocsStatus::Synced, "up to date".to_string())
            };

            CrateStatus {
                crate_name,
                lock_version: Some(lock_version),
                docs_version: Some(docs_version),
                status,
                reason,
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
    print!("{}", format_status_table(statuses));
}

fn format_status_table(statuses: &[CrateStatus]) -> String {
    const COL_CRATE: usize = 24;
    const COL_LOCK: usize = 16;
    const COL_DOCS: usize = 16;
    const COL_STATUS: usize = 14;

    let mut output = String::new();
    let _ = writeln!(
        output,
        "{:<COL_CRATE$} {:<COL_LOCK$} {:<COL_DOCS$} {:<COL_STATUS$}",
        "Crate", "Lock Version", "Docs Version", "Status"
    );
    let _ = writeln!(
        output,
        "{:-<COL_CRATE$} {:-<COL_LOCK$} {:-<COL_DOCS$} {:-<COL_STATUS$}",
        "", "", "", ""
    );

    for item in statuses {
        let lock = item.lock_version.as_deref().unwrap_or("-");
        let docs = item.docs_version.as_deref().unwrap_or("-");
        let _ = writeln!(
            output,
            "{:<COL_CRATE$} {:<COL_LOCK$} {:<COL_DOCS$} {:<COL_STATUS$}",
            item.crate_name,
            lock,
            docs,
            item.status.as_str(),
        );
        let _ = writeln!(output, "  ↳ {}", item.reason);
    }

    let summary = summarize(statuses);
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "Total: {} | Synced: {} | Missing: {} | Outdated: {} | Corrupted: {}",
        summary.total, summary.synced, summary.missing, summary.outdated, summary.corrupted
    );

    if summary.has_problems() {
        let _ = writeln!(
            output,
            "Hint: run `cargo ai-fdocs sync` (or `--force` for full refresh)"
        );
        let _ = writeln!(
            output,
            "CI hint: run `cargo ai-fdocs check` to fail on stale docs"
        );

        let _ = writeln!(output, "\nProblem details:");
        for item in statuses.iter().filter(|s| s.status.is_problem()) {
            let _ = writeln!(
                output,
                "- {} [{}]: {}",
                item.crate_name,
                item.status.as_str(),
                item.reason
            );
        }
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

#[cfg(test)]
mod tests {
    use super::{format_status_table, CrateStatus, DocsStatus};

    #[test]
    fn formats_empty_status_table_with_zero_summary() {
        let table = format_status_table(&[]);

        assert!(table.contains("Crate"));
        assert!(table.contains("Lock Version"));
        assert!(table.contains("Docs Version"));
        assert!(table.contains("Status"));
        assert!(table.contains("Total: 0 | Synced: 0 | Missing: 0 | Outdated: 0 | Corrupted: 0"));
        assert!(!table.contains("Hint: run `cargo ai-fdocs sync`"));
    }

    #[test]
    fn formats_missing_lock_version_and_shows_hints_and_problem_details() {
        let statuses = vec![CrateStatus {
            crate_name: "serde".to_string(),
            lock_version: None,
            docs_version: None,
            status: DocsStatus::Missing,
            reason: "crate missing in Cargo.lock".to_string(),
        }];

        let table = format_status_table(&statuses);

        assert!(table.contains("serde"));
        assert!(table.contains("Missing"));
        assert!(table.contains("crate missing in Cargo.lock"));
        assert!(table.contains("Hint: run `cargo ai-fdocs sync`"));
        assert!(table.contains("CI hint: run `cargo ai-fdocs check`"));
        assert!(table.contains("Problem details:"));
    }
}
