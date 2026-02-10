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
