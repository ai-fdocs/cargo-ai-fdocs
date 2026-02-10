mod config;
mod error;
#[path = "fetcher/mod.rs"]
mod fetcher;
mod index;
mod init;
mod processor;
mod resolver;
mod status;
mod storage;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Semaphore;

use clap::{Parser, Subcommand, ValueEnum};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::error::AiDocsError;
use crate::error::{Result, SyncErrorKind};
use crate::fetcher::github::{FetchedFile, FileRequest, GitHubFetcher};
use crate::init::run_init as run_init_command;
use crate::status::{collect_status, print_status_table, DocsStatus};

const DEFAULT_CONFIG_PATH: &str = "ai-fdocs.toml";

#[derive(Parser)]
#[command(name = "cargo-ai-fdocs")]
#[command(bin_name = "cargo")]
enum CargoCli {
    #[command(name = "ai-fdocs")]
    AiFdocs(Cli),
}

#[derive(Parser)]
#[command(version, about = "Sync documentation from dependencies for AI context")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download/update vendor documentation
    Sync {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
        /// Ignore local cache and re-fetch configured docs.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Show documentation sync status for configured crates.
    Status {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
        /// Output format for status report.
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Exit non-zero if any crate docs are not synced.
    Check {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
        /// Output format for check report.
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Generate or refresh ai-fdocs config template.
    Init {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
        /// Overwrite existing config file.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

#[derive(Default)]
struct SyncStats {
    synced: usize,
    cached: usize,
    skipped: usize,
    errors: usize,
    auth_errors: usize,
    rate_limit_errors: usize,
    network_errors: usize,
    not_found_errors: usize,
    other_errors: usize,
}

impl SyncStats {
    fn record_error(&mut self, kind: SyncErrorKind) {
        self.errors += 1;
        match kind {
            SyncErrorKind::Auth => self.auth_errors += 1,
            SyncErrorKind::RateLimit => self.rate_limit_errors += 1,
            SyncErrorKind::Network => self.network_errors += 1,
            SyncErrorKind::NotFound => self.not_found_errors += 1,
            SyncErrorKind::Other => self.other_errors += 1,
        }
    }
}

#[derive(Debug)]
enum SyncOutcome {
    Synced(storage::SavedCrate),
    Cached(Option<storage::SavedCrate>),
    Skipped,
    Error(SyncErrorKind),
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = std::env::args()
        .enumerate()
        .filter(|(i, arg)| !(*i == 1 && arg == "ai-fdocs"))
        .map(|(_, arg)| arg)
        .collect();

    let parse = CargoCli::try_parse_from(args).map(|parsed| match parsed {
        CargoCli::AiFdocs(cli) => cli,
    });

    let cli = match parse {
        Ok(cli) => cli,
        Err(e) => {
            e.print().expect("failed to print clap error");
            std::process::exit(2);
        }
    };

    if let Err(e) = run(cli).await {
        error!("{e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Sync { config, force } => run_sync(&config, force).await,
        Commands::Status { config, format } => run_status(&config, format),
        Commands::Check { config, format } => run_check(&config, format),
        Commands::Init { config, force } => run_init_command(&config, force).await,
    }
}

async fn run_sync(config_path: &Path, force: bool) -> Result<()> {
    let config = Config::load(config_path)?;
    info!("Loaded config from {}", config_path.display());

    let cargo_lock_path = PathBuf::from("Cargo.lock");
    let rust_versions = resolver::resolve_cargo_versions(&cargo_lock_path)?;

    let rust_output_dir = storage::rust_output_dir(&config.settings.output_dir);
    if config.settings.prune {
        storage::prune(&rust_output_dir, &config, &rust_versions)?;
    }

    let fetcher = Arc::new(GitHubFetcher::new());
    let mut saved_crates = Vec::new();
    let mut stats = SyncStats::default();

    let mut jobs = Vec::new();
    for (crate_name, crate_doc) in &config.crates {
        jobs.push((crate_name.clone(), crate_doc.clone()));
    }

    let max_file_size_kb = config.settings.max_file_size_kb;
    let concurrency = config.settings.sync_concurrency;
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut join_set = tokio::task::JoinSet::new();

    for (crate_name, crate_doc) in jobs {
        let rust_output_dir = rust_output_dir.clone();
        let rust_versions = rust_versions.clone();
        let fetcher = Arc::clone(&fetcher);
        let semaphore = Arc::clone(&semaphore);

        join_set.spawn(async move {
            let _permit = semaphore.acquire_owned().await.expect("semaphore closed");
            sync_one_crate(
                rust_output_dir,
                rust_versions,
                fetcher,
                crate_name,
                crate_doc,
                force,
                max_file_size_kb,
            )
            .await
        });
    }

    while let Some(joined) = join_set.join_next().await {
        let result = match joined {
            Ok(result) => result,
            Err(e) => {
                warn!("sync worker failed: {e}");
                SyncOutcome::Error(SyncErrorKind::Other)
            }
        };
        match result {
            SyncOutcome::Synced(saved) => {
                saved_crates.push(saved);
                stats.synced += 1;
            }
            SyncOutcome::Cached(saved) => {
                if let Some(saved) = saved {
                    saved_crates.push(saved);
                }
                stats.cached += 1;
            }
            SyncOutcome::Skipped => stats.skipped += 1,
            SyncOutcome::Error(kind) => stats.record_error(kind),
        }
    }

    index::generate_index(&rust_output_dir, &saved_crates)?;

    info!(
        "✅ Sync complete: {} synced, {} cached, {} skipped, {} errors",
        stats.synced, stats.cached, stats.skipped, stats.errors
    );

    if stats.errors > 0 {
        info!(
            "   error breakdown: auth={}, rate-limit={}, network={}, not-found={}, other={}",
            stats.auth_errors,
            stats.rate_limit_errors,
            stats.network_errors,
            stats.not_found_errors,
            stats.other_errors
        );
    }

    Ok(())
}

async fn sync_one_crate(
    rust_output_dir: PathBuf,
    rust_versions: std::collections::HashMap<String, String>,
    fetcher: Arc<GitHubFetcher>,
    crate_name: String,
    crate_doc: crate::config::CrateDoc,
    force: bool,
    max_file_size_kb: usize,
) -> SyncOutcome {
    let Some(version) = rust_versions.get(crate_name.as_str()).cloned() else {
        warn!("Crate '{crate_name}' not found in Cargo.lock, skipping");
        return SyncOutcome::Skipped;
    };

    let Some(repo) = crate_doc.github_repo().map(str::to_string) else {
        warn!("Crate '{crate_name}' has no GitHub repo in config, skipping");
        return SyncOutcome::Skipped;
    };

    if !force && storage::is_cached(&rust_output_dir, &crate_name, &version, &crate_doc) {
        info!("  ⏭ {crate_name}@{version}: cached, skipping");
        let cached = storage::read_cached_info(&rust_output_dir, &crate_name, &version, &crate_doc);
        return SyncOutcome::Cached(cached);
    }

    info!("Syncing {crate_name}@{version}...");

    let resolved = match fetcher
        .resolve_ref(&repo, &crate_name, version.as_str())
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("  ✗ failed to resolve ref for {crate_name}@{version}: {e}");
            return SyncOutcome::Error(e.sync_kind());
        }
    };

    if resolved.is_fallback {
        warn!(
            "  ⚠ no exact tag for {crate_name}@{version}, using {}",
            resolved.git_ref
        );
    }

    let requests = build_requests(crate_doc.subpath.as_deref(), crate_doc.effective_files());
    let results = fetcher
        .fetch_files(&repo, &resolved.git_ref, &requests)
        .await;

    let fetched = collect_fetched_files(results, &crate_name, &version);
    if fetched.non_optional_errors > 0 && !fetched.files.is_empty() {
        warn!(
            "  ⚠ partial fetch for {crate_name}@{version}: {} file error(s), continuing with {} fetched file(s)",
            fetched.non_optional_errors,
            fetched.files.len()
        );
    }

    if fetched.files.is_empty() {
        warn!("  ✗ no files fetched for {crate_name}@{version}");
        return SyncOutcome::Error(SyncErrorKind::NotFound);
    }

    let save_ctx = storage::SaveContext {
        repo: &repo,
        resolved: &resolved,
        max_file_size_kb,
    };

    match storage::save_crate_files(
        &rust_output_dir,
        &crate_name,
        &version,
        &save_ctx,
        &fetched.files,
        &crate_doc,
    ) {
        Ok(saved) => SyncOutcome::Synced(saved),
        Err(e) => {
            warn!("  ✗ failed to save {crate_name}@{version}: {e}");
            SyncOutcome::Error(e.sync_kind())
        }
    }
}

struct FetchCollection {
    files: Vec<FetchedFile>,
    non_optional_errors: usize,
}

fn collect_fetched_files(
    results: Vec<Result<FetchedFile>>,
    crate_name: &str,
    version: &str,
) -> FetchCollection {
    let mut files = Vec::new();
    let mut non_optional_errors = 0;

    for r in results {
        match r {
            Ok(file) => files.push(file),
            Err(e) => match e {
                AiDocsError::OptionalFileNotFound(_) => {}
                other => {
                    non_optional_errors += 1;
                    warn!("  ✗ {crate_name}@{version}: {other}");
                }
            },
        }
    }

    FetchCollection {
        files,
        non_optional_errors,
    }
}

fn build_requests(subpath: Option<&str>, explicit_files: Option<Vec<String>>) -> Vec<FileRequest> {
    if let Some(files) = explicit_files {
        return files
            .into_iter()
            .map(|f| FileRequest {
                original_path: f.clone(),
                candidates: vec![f],
                required: true,
            })
            .collect();
    }

    let prefix = subpath
        .map(|s| s.trim_matches('/'))
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s}/"))
        .unwrap_or_default();

    vec![
        FileRequest {
            original_path: format!("{prefix}README.md"),
            candidates: vec![
                format!("{prefix}README.md"),
                format!("{prefix}Readme.md"),
                format!("{prefix}readme.md"),
            ],
            required: false,
        },
        FileRequest {
            original_path: format!("{prefix}CHANGELOG.md"),
            candidates: vec![
                format!("{prefix}CHANGELOG.md"),
                format!("{prefix}Changelog.md"),
                format!("{prefix}changelog.md"),
            ],
            required: false,
        },
    ]
}

fn should_emit_plain_check_errors(format: OutputFormat, github_actions: bool) -> bool {
    !github_actions && matches!(format, OutputFormat::Table)
}

fn emit_check_failures_for_ci(format: OutputFormat, statuses: &[crate::status::CrateStatus]) {
    let github_actions = std::env::var("GITHUB_ACTIONS")
        .ok()
        .map(|v| v == "true")
        .unwrap_or(false);

    for status in statuses
        .iter()
        .filter(|s| !matches!(s.status, DocsStatus::Synced | DocsStatus::SyncedFallback))
    {
        if github_actions {
            eprintln!(
                "::error title=ai-fdocs check::{} [{}] {}",
                status.crate_name,
                status.status.as_str(),
                status.reason
            );
        } else if should_emit_plain_check_errors(format, github_actions) {
            eprintln!(
                "[ai-fdocs check] {} [{}] {}",
                status.crate_name,
                status.status.as_str(),
                status.reason
            );
        }
    }
}

fn print_statuses(format: OutputFormat, statuses: &[crate::status::CrateStatus]) -> Result<()> {
    match format {
        OutputFormat::Table => print_status_table(statuses),
        OutputFormat::Json => {
            let json = status::format_status_json(statuses).map_err(|e| {
                error::AiDocsError::Other(format!("failed to serialize status JSON: {e}"))
            })?;
            println!("{json}");
        }
    }

    Ok(())
}

fn run_status(config_path: &Path, format: OutputFormat) -> Result<()> {
    let config = Config::load(config_path)?;
    let rust_versions = resolver::resolve_cargo_versions(PathBuf::from("Cargo.lock").as_path())?;

    let rust_dir = storage::rust_output_dir(&config.settings.output_dir);
    let statuses = collect_status(&config, &rust_versions, &rust_dir);
    print_statuses(format, &statuses)
}

fn run_check(config_path: &Path, format: OutputFormat) -> Result<()> {
    let config = Config::load(config_path)?;
    let rust_versions = resolver::resolve_cargo_versions(PathBuf::from("Cargo.lock").as_path())?;
    let rust_dir = storage::rust_output_dir(&config.settings.output_dir);

    let statuses = collect_status(&config, &rust_versions, &rust_dir);
    let failing = statuses
        .iter()
        .any(|s| !matches!(s.status, DocsStatus::Synced | DocsStatus::SyncedFallback));

    if failing {
        print_statuses(format, &statuses)?;
        emit_check_failures_for_ci(format, &statuses);
        return Err(error::AiDocsError::Other(
            "Documentation is outdated, missing, or corrupted. Run: cargo ai-fdocs sync"
                .to_string(),
        ));
    }

    match format {
        OutputFormat::Table => info!("All configured crate docs are up to date."),
        OutputFormat::Json => print_statuses(format, &statuses)?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_requests, collect_fetched_files, should_emit_plain_check_errors, OutputFormat,
    };
    use crate::error::AiDocsError;
    use crate::fetcher::github::FetchedFile;
    use clap::CommandFactory;

    #[test]
    fn emits_plain_errors_only_for_table_outside_gha() {
        assert!(should_emit_plain_check_errors(OutputFormat::Table, false));
        assert!(!should_emit_plain_check_errors(OutputFormat::Json, false));
    }

    #[test]
    fn never_emits_plain_errors_in_github_actions() {
        assert!(!should_emit_plain_check_errors(OutputFormat::Table, true));
        assert!(!should_emit_plain_check_errors(OutputFormat::Json, true));
    }

    #[test]
    fn build_requests_prefers_explicit_files_and_marks_them_required() {
        let requests = build_requests(
            Some("docs"),
            Some(vec!["README.md".to_string(), "guide/intro.md".to_string()]),
        );

        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.required));
        assert_eq!(requests[0].candidates, vec!["README.md"]);
        assert_eq!(requests[1].candidates, vec!["guide/intro.md"]);
    }

    #[test]
    fn collect_fetched_files_keeps_successes_on_partial_failures() {
        let results = vec![
            Ok(FetchedFile {
                path: "README.md".to_string(),
                source_url: "https://example.invalid/readme".to_string(),
                content: "hello".to_string(),
            }),
            Err(AiDocsError::OptionalFileNotFound(
                "CHANGELOG.md".to_string(),
            )),
            Err(AiDocsError::GitHubFileNotFound {
                repo: "owner/repo".to_string(),
                path: "docs/guide.md".to_string(),
                tried_tags: vec!["v1.0.0".to_string()],
            }),
        ];

        let kept = collect_fetched_files(results, "demo", "1.0.0");
        assert_eq!(kept.files.len(), 1);
        assert_eq!(kept.files[0].path, "README.md");
        assert_eq!(kept.non_optional_errors, 1);
    }

    #[test]
    fn collect_fetched_files_counts_only_non_optional_errors() {
        let results = vec![
            Err(AiDocsError::OptionalFileNotFound("README.md".to_string())),
            Err(AiDocsError::OptionalFileNotFound(
                "CHANGELOG.md".to_string(),
            )),
        ];

        let kept = collect_fetched_files(results, "demo", "1.0.0");
        assert!(kept.files.is_empty());
        assert_eq!(kept.non_optional_errors, 0);
    }

    #[test]
    fn cli_subcommands_have_consistent_help_and_config_flag() {
        let mut command = super::CargoCli::command();
        command.build();

        let ai_fdocs_cmd = command
            .find_subcommand("ai-fdocs")
            .expect("ai-fdocs subcommand present");

        for sub in ["sync", "status", "check", "init"] {
            let sub_cmd = ai_fdocs_cmd
                .find_subcommand(sub)
                .unwrap_or_else(|| panic!("missing subcommand: {sub}"));

            assert!(
                sub_cmd.get_about().is_some(),
                "subcommand should have help text: {sub}"
            );

            let has_config = sub_cmd
                .get_arguments()
                .any(|arg| arg.get_id().as_str() == "config");
            assert!(has_config, "subcommand should expose --config: {sub}");
        }
    }
}
