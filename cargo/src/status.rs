use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusMode {
    Lockfile,
    LatestDocs,
}

impl StatusMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Lockfile => "lockfile",
            Self::LatestDocs => "latest_docs",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrateStatus {
    pub crate_name: String,
    pub lock_version: Option<String>,
    pub docs_version: Option<String>,
    pub status: DocsStatus,
    pub reason: String,
    pub reason_code: String,
    pub mode: String,
    pub source_kind: Option<String>,
}

use crate::storage::CrateMeta;

fn crate_status(
    crate_name: String,
    lock_version: Option<String>,
    docs_version: Option<String>,
    status: DocsStatus,
    reason: impl Into<String>,
    reason_code: impl Into<String>,
    mode: StatusMode,
    source_kind: Option<String>,
) -> CrateStatus {
    CrateStatus {
        crate_name,
        lock_version,
        docs_version,
        status,
        reason: reason.into(),
        reason_code: reason_code.into(),
        mode: mode.as_str().to_string(),
        source_kind,
    }
}

pub async fn collect_status(
    config: &Config,
    lock_versions: &HashMap<String, String>,
    output_dir: &Path,
) -> Vec<CrateStatus> {
    let mut crate_names: Vec<_> = config.crates.keys().cloned().collect();
    crate_names.sort();

    let existing_map = scan_existing_dirs(output_dir);

    let mut results = Vec::new();
    for crate_name in crate_names {
        let status = if let Some(lock_version) = lock_versions.get(&crate_name).cloned() {
            let expected_dir = output_dir.join(format!("{crate_name}@{lock_version}"));
            if !expected_dir.is_dir() {
                let docs_version = existing_map.get(&crate_name).map(|(v, _)| v.clone());
                if let Some(existing) = docs_version {
                    crate_status(
                        crate_name,
                        Some(lock_version.clone()),
                        Some(existing.clone()),
                        DocsStatus::Outdated,
                        format!(
                            "cached docs version {existing} differs from lock version {lock_version}"
                        ),
                        "lockfile_version_mismatch",
                        StatusMode::Lockfile,
                        None,
                    )
                } else {
                    crate_status(
                        crate_name,
                        Some(lock_version),
                        None,
                        DocsStatus::Missing,
                        "no synced docs found for this crate",
                        "lockfile_missing_artifacts",
                        StatusMode::Lockfile,
                        None,
                    )
                }
            } else {
                let meta_path = expected_dir.join(".aifd-meta.toml");
                match std::fs::read_to_string(&meta_path) {
                    Ok(meta_raw) => {
                        match toml::from_str::<CrateMeta>(&meta_raw) {
                            Ok(meta) => {
                                if meta.schema_version > 1 {
                                    crate_status(
                                        crate_name,
                                        Some(lock_version.clone()),
                                        Some(lock_version),
                                        DocsStatus::Corrupted,
                                        format!(
                                            ".aifd-meta.toml schema version {} is newer than supported version 1",
                                            meta.schema_version
                                        ),
                                        "meta_schema_unsupported",
                                        StatusMode::Lockfile,
                                        meta.source_kind.clone(),
                                    )
                                } else {
                                    let docs_version = meta.version.clone();
                                    if docs_version != lock_version {
                                        crate_status(
                                            crate_name,
                                            Some(lock_version.clone()),
                                            Some(docs_version.clone()),
                                            DocsStatus::Outdated,
                                            format!("metadata version {docs_version} differs from lock version {lock_version}"),
                                            "meta_version_mismatch",
                                            StatusMode::Lockfile,
                                            meta.source_kind.clone(),
                                        )
                                    } else if meta.is_fallback {
                                        crate_status(
                                            crate_name,
                                            Some(lock_version),
                                            Some(docs_version),
                                            DocsStatus::SyncedFallback,
                                            "synced from fallback branch (no exact tag found)",
                                            "lockfile_fallback_branch",
                                            StatusMode::Lockfile,
                                            Some("github_fallback".to_string()),
                                        )
                                    } else {
                                        crate_status(
                                            crate_name,
                                            Some(lock_version),
                                            Some(docs_version),
                                            DocsStatus::Synced,
                                            "up to date",
                                            "lockfile_ok",
                                            StatusMode::Lockfile,
                                            Some("github".to_string()),
                                        )
                                    }
                                }
                            }
                            Err(_) => {
                                crate_status(
                                    crate_name,
                                    Some(lock_version.clone()),
                                    Some(lock_version),
                                    DocsStatus::Corrupted,
                                    ".aifd-meta.toml has invalid TOML",
                                    "meta_invalid_toml",
                                    StatusMode::Lockfile,
                                    None,
                                )
                            }
                        }
                    }
                    Err(_) => {
                        crate_status(
                            crate_name,
                            Some(lock_version.clone()),
                            Some(lock_version),
                            DocsStatus::Corrupted,
                            ".aifd-meta.toml is missing or unreadable",
                            "meta_unreadable",
                            StatusMode::Lockfile,
                            None,
                        )
                    }
                }
            }
        } else {
            crate_status(
                crate_name,
                None,
                None,
                DocsStatus::Missing,
                "crate missing in Cargo.lock",
                "lockfile_missing_crate",
                StatusMode::Lockfile,
                None,
            )
        };
        results.push(status);
    }
    results
}

pub async fn collect_status_latest(
    config: &Config,
    output_dir: &Path,
    fetcher: Option<&crate::fetcher::latest::LatestDocsFetcher>,
) -> Vec<CrateStatus> {
    let mut crate_names: Vec<_> = config.crates.keys().cloned().collect();
    crate_names.sort();

    let existing_map = scan_existing_dirs(output_dir);

    let mut results = Vec::new();
    for crate_name in crate_names {
        let status = if let Some((docs_version, crate_dir)) = existing_map.get(&crate_name) {
            let docs_version = docs_version.clone();
            let meta_path = crate_dir.join(".aifd-meta.toml");
            match std::fs::read_to_string(&meta_path) {
                Ok(meta_raw) => {
                    match toml::from_str::<CrateMeta>(&meta_raw) {
                        Ok(meta) => {
                            if meta.schema_version > 1 {
                                crate_status(
                                    crate_name.clone(),
                                    None,
                                    Some(docs_version),
                                    DocsStatus::Corrupted,
                                    format!(
                                        ".aifd-meta.toml schema version {} is newer than supported version 1",
                                        meta.schema_version
                                    ),
                                    "meta_schema_unsupported",
                                    StatusMode::LatestDocs,
                                    meta.source_kind.clone(),
                                )
                            } else {
                                let source_kind = meta.source_kind.clone().unwrap_or_else(|| "docsrs".to_string());
                                let is_fallback = meta.is_fallback || source_kind == "github_fallback";

                                // Check freshness if fetcher is provided
                                let mut status = if is_fallback {
                                    DocsStatus::SyncedFallback
                                } else {
                                    DocsStatus::Synced
                                };
                                let mut reason = if is_fallback {
                                    "latest-docs synced via GitHub fallback".to_string()
                                } else {
                                    "latest-docs up to date".to_string()
                                };
                                let mut reason_code = if is_fallback {
                                    "latest_ok_fallback".to_string()
                                } else {
                                    "latest_ok_docsrs".to_string()
                                };

                                if let Some(f) = fetcher {
                                    let mut needs_check = true;
                                    if let Some(checked_at) = &meta.upstream_checked_at {
                                        if crate::utils::is_latest_cache_fresh(checked_at, config.settings.latest_ttl_hours) {
                                            needs_check = false;
                                        }
                                    }

                                    if needs_check {
                                        if let Ok(latest) = f.resolve_latest_version(&crate_name).await {
                                            if latest != docs_version {
                                                status = DocsStatus::Outdated;
                                                reason = format!("latest version {latest} is newer than cached {docs_version}");
                                                reason_code = "latest_version_mismatch".to_string();
                                            }
                                        }
                                    }
                                }

                                crate_status(
                                    crate_name.clone(),
                                    None,
                                    Some(docs_version),
                                    status,
                                    reason,
                                    reason_code,
                                    StatusMode::LatestDocs,
                                    Some(source_kind),
                                )
                            }
                        }
                        Err(_) => {
                            crate_status(
                                crate_name.clone(),
                                None,
                                Some(docs_version),
                                DocsStatus::Corrupted,
                                ".aifd-meta.toml has invalid TOML",
                                "meta_invalid_toml",
                                StatusMode::LatestDocs,
                                None,
                            )
                        }
                    }
                }
                Err(_) => {
                    crate_status(
                        crate_name.clone(),
                        None,
                        Some(docs_version),
                        DocsStatus::Corrupted,
                        ".aifd-meta.toml is missing or unreadable",
                        "meta_unreadable",
                        StatusMode::LatestDocs,
                        None,
                    )
                }
            }
        } else {
            crate_status(
                crate_name.clone(),
                None,
                None,
                DocsStatus::Missing,
                "no synced docs found for this crate",
                "latest_missing_artifacts",
                StatusMode::LatestDocs,
                None,
            )
        };
        results.push(status);
    }
    results
}

fn scan_existing_dirs(output_dir: &Path) -> HashMap<String, (String, PathBuf)> {
    let mut map: HashMap<String, (String, PathBuf)> = HashMap::new();
    
    let Ok(entries) = std::fs::read_dir(output_dir) else {
        return map;
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }

        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();
        
        if let Some((name, version)) = split_crate_version(&dir_name) {
            let entry_v = version.to_string();
            let entry_p = entry.path();
            
            if let Some((best_v, _)) = map.get(name) {
                if crate::utils::is_version_better(&entry_v, Some(best_v)) {
                    map.insert(name.to_string(), (entry_v, entry_p));
                }
            } else {
                map.insert(name.to_string(), (entry_v, entry_p));
            }
        }
    }
    map
}

fn split_crate_version(dir_name: &str) -> Option<(&str, &str)> {
    dir_name.rsplit_once('@')
}



pub fn print_status_table(statuses: &[CrateStatus]) {
    print!("{}", format_status_table(statuses));
}

#[derive(Debug, Serialize)]
struct StatusReport<'a> {
    summary: StatusSummary,
    statuses: &'a [CrateStatus],
}

pub fn format_status_json(
    statuses: &[CrateStatus],
) -> std::result::Result<String, serde_json::Error> {
    let report = StatusReport {
        summary: summarize(statuses),
        statuses,
    };

    serde_json::to_string_pretty(&report)
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

#[derive(Debug, Default, Serialize)]
pub struct StatusSummary {
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

pub fn summarize(statuses: &[CrateStatus]) -> StatusSummary {
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
    use super::{
        collect_status_latest, format_status_json, format_status_table, CrateStatus, DocsStatus,
        StatusMode,
    };
    use crate::config::{Config, CrateDoc, Settings};
    use std::collections::HashMap;
    use std::fs;

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
    fn formats_status_json_with_summary_and_entries() {
        let statuses = vec![CrateStatus {
            crate_name: "axum".to_string(),
            lock_version: Some("0.8.1".to_string()),
            docs_version: Some("0.8.1".to_string()),
            status: DocsStatus::Synced,
            reason: "up to date".to_string(),
            reason_code: "lockfile_ok".to_string(),
            mode: StatusMode::Lockfile.as_str().to_string(),
            source_kind: Some("github".to_string()),
        }];

        let json = format_status_json(&statuses).expect("json serialization");

        assert!(json.contains("\"summary\""));
        assert!(json.contains("\"statuses\""));
        assert!(json.contains("\"crate_name\": \"axum\""));
        assert!(json.contains("\"status\": \"Synced\""));
        assert!(json.contains("\"reason_code\": \"lockfile_ok\""));
        assert!(json.contains("\"mode\": \"lockfile\""));
        assert!(json.contains("\"source_kind\": \"github\""));
    }

    #[test]
    fn formats_missing_lock_version_and_shows_hints_and_problem_details() {
        let statuses = vec![CrateStatus {
            crate_name: "serde".to_string(),
            lock_version: None,
            docs_version: None,
            status: DocsStatus::Missing,
            reason: "crate missing in Cargo.lock".to_string(),
            reason_code: "lockfile_missing_crate".to_string(),
            mode: StatusMode::Lockfile.as_str().to_string(),
            source_kind: None,
        }];

        let table = format_status_table(&statuses);

        assert!(table.contains("serde"));
        assert!(table.contains("Missing"));
        assert!(table.contains("crate missing in Cargo.lock"));
        assert!(table.contains("Hint: run `cargo ai-fdocs sync`"));
        assert!(table.contains("CI hint: run `cargo ai-fdocs check`"));
        assert!(table.contains("Problem details:"));
    }

    #[test]
    fn collect_status_latest_marks_github_fallback_as_synced_fallback() {
        let tmp = std::env::temp_dir().join(format!("aifd-status-latest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("serde@1.0.0")).expect("create crate dir");
        fs::write(
            tmp.join("serde@1.0.0/.aifd-meta.toml"),
            "schema_version = 1\nversion = \"1.0.0\"\nsource_kind = \"github_fallback\"\n",
        )
        .expect("write meta");

        let mut crates = HashMap::new();
        crates.insert(
            "serde".to_string(),
            CrateDoc {
                repo: Some("serde-rs/serde".to_string()),
                subpath: None,
                files: None,
                sources: None,
                ai_notes: String::new(),
            },
        );

        let config = Config {
            settings: Settings::default(),
            crates,
        };

        let statuses = collect_status_latest(&config, tmp.as_path());
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].status, DocsStatus::SyncedFallback);
        assert_eq!(statuses[0].reason_code, "latest_ok_fallback");
        assert_eq!(statuses[0].mode, "latest_docs");
        assert_eq!(statuses[0].source_kind.as_deref(), Some("github_fallback"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
