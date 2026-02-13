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
mod utils;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Semaphore;

use chrono::{NaiveDate, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use tracing::{error, info, warn};

use crate::config::{Config, DocsSource, SyncMode};
use crate::error::AiDocsError;
use crate::error::{Result, SyncErrorKind};
use crate::fetcher::github::{FetchedFile, FileRequest, GitHubFetcher};
use crate::fetcher::latest::{is_docsrs_fallback_eligible, LatestDocsFetcher};
use crate::init::run_init as run_init_command;
use crate::status::{collect_status, collect_status_latest, print_status_table, DocsStatus};

const DEFAULT_CONFIG_PATH: &str = "ai-fdocs.toml";

#[derive(Parser)]
#[command(name = "ai-fdocs")]
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
        /// Sync mode override (`lockfile` is stable default, `latest-docs` is beta).
        #[arg(long, value_enum)]
        mode: Option<SyncModeArg>,
        /// Ignore local cache and re-fetch configured docs.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Show documentation sync status for configured crates.
    Status {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
        /// Sync mode override for status evaluation.
        #[arg(long, value_enum)]
        mode: Option<SyncModeArg>,
        /// Output format for status report.
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Exit non-zero if any crate docs are not synced.
    Check {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
        /// Sync mode override for check evaluation.
        #[arg(long, value_enum)]
        mode: Option<SyncModeArg>,
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
enum SyncModeArg {
    Lockfile,
    LatestDocs,
    Hybrid,
}

impl SyncModeArg {
    const fn to_sync_mode(self) -> SyncMode {
        match self {
            Self::Lockfile => SyncMode::Lockfile,
            Self::LatestDocs => SyncMode::LatestDocs,
            Self::Hybrid => SyncMode::Hybrid,
        }
    }
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

    let parse = Cli::try_parse_from(args);

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
        Commands::Sync {
            config,
            mode,
            force,
        } => run_sync(&config, mode, force).await,
        Commands::Status {
            config,
            mode,
            format,
        } => run_status(&config, mode, format).await,
        Commands::Check {
            config,
            mode,
            format,
        } => run_check(&config, mode, format).await,
        Commands::Init { config, force } => run_init_command(&config, force).await,
    }
}

async fn run_sync(
    config_path: &Path,
    mode_override: Option<SyncModeArg>,
    force: bool,
) -> Result<()> {
    let config = Config::load(config_path)?;
    info!("Loaded config from {}", config_path.display());

    let sync_mode = resolve_sync_mode(mode_override, config.settings.sync_mode);
    info!("Resolved sync mode: {}", sync_mode.as_str());
    if matches!(sync_mode, SyncMode::LatestDocs) {
        return run_sync_latest_docs(config, force).await;
    }

    match config.settings.docs_source {
        DocsSource::GitHub => info!("Using docs source: github"),
    }

    let cargo_lock_path = PathBuf::from("Cargo.lock");
    let rust_versions = resolver::resolve_cargo_versions(&cargo_lock_path)?;

    let rust_output_dir = storage::rust_output_dir(&config.settings.output_dir);
    if config.settings.prune {
        storage::prune(&rust_output_dir, &config, &rust_versions)?;
    }

    let fetcher = Arc::new(GitHubFetcher::new());
    let mut saved_crates = Vec::new();
    let mut stats = SyncStats::default();

    let outcomes = run_orchestrated_sync(
        &config,
        config.crates.iter().map(|(n, c)| (n.clone(), c.clone())).collect(),
        |crate_name, crate_doc| {
            let rust_output_dir = rust_output_dir.clone();
            let rust_versions = rust_versions.clone();
            let fetcher = Arc::clone(&fetcher);
            let force = force;
            let max_file_size_kb = max_file_size_kb;
            async move {
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
            }
        },
    )
    .await;

    for result in outcomes {
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

async fn run_sync_latest_docs(config: Config, force: bool) -> Result<()> {
    info!("Using docs source: crates.io + docs.rs (with GitHub fallback)");

    let rust_output_dir = storage::rust_output_dir(&config.settings.output_dir);
    let github_fetcher = Arc::new(GitHubFetcher::new());
    let latest_fetcher = Arc::new(LatestDocsFetcher::new());

    let mut saved_crates = Vec::new();
    let mut stats = SyncStats::default();

    let outcomes = run_orchestrated_sync(
        &config,
        config.crates.clone().into_iter().collect(),
        |crate_name, crate_doc| {
            let rust_output_dir = rust_output_dir.clone();
            let github_fetcher = Arc::clone(&github_fetcher);
            let latest_fetcher = Arc::clone(&latest_fetcher);
            let max_file_size_kb = config.settings.max_file_size_kb;
            let ttl = config.settings.latest_ttl_hours;
            async move {
                sync_one_crate_latest(
                    rust_output_dir,
                    latest_fetcher,
                    github_fetcher,
                    crate_name,
                    crate_doc,
                    force,
                    max_file_size_kb,
                    ttl,
                )
                .await
            }
        },
    )
    .await;

    for outcome in outcomes {
        match outcome {
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
        "✅ Latest-docs sync complete: {} synced, {} cached, {} skipped, {} errors",
        stats.synced, stats.cached, stats.skipped, stats.errors
    );

    Ok(())
}

async fn sync_one_crate_latest(
    rust_output_dir: PathBuf,
    latest_fetcher: Arc<LatestDocsFetcher>,
    github_fetcher: Arc<GitHubFetcher>,
    crate_name: String,
    crate_doc: crate::config::CrateDoc,
    force: bool,
    max_file_size_kb: usize,
    latest_ttl_hours: usize,
) -> SyncOutcome {
    let version = match latest_fetcher.resolve_latest_version(&crate_name).await {
        Ok(v) => v,
        Err(e) => {
            warn!("  ✗ failed to resolve latest version for {crate_name}: {e}");
            return SyncOutcome::Error(e.sync_kind());
        }
    };

    if !force && storage::is_cached(&rust_output_dir, &crate_name, &version, &crate_doc) {
        if let Some(meta) = storage::read_meta(&rust_output_dir, &crate_name, &version) {
            if is_latest_cache_fresh(&meta.fetched_at, latest_ttl_hours) {
                info!("  ⏭ {crate_name}@{version}: cached (TTL valid), skipping");
                let cached =
                    storage::read_cached_info(&rust_output_dir, &crate_name, &version, &crate_doc);
                return SyncOutcome::Cached(cached);
            }
            info!("  🔄 {crate_name}@{version}: cache TTL expired, refreshing");
        }
    }

    match latest_fetcher
        .fetch_api_markdown(&crate_name, &version, max_file_size_kb)
        .await
    {
        Ok(artifact) => match storage::save_latest_api_markdown(
            &rust_output_dir,
            &crate_name,
            &version,
            &artifact.markdown,
            &artifact.docsrs_input_url,
            artifact.truncated,
            &crate_doc,
        ) {
            Ok(saved) => SyncOutcome::Synced(saved),
            Err(e) => {
                warn!("  ✗ failed to save docs.rs artifact for {crate_name}@{version}: {e}");
                SyncOutcome::Error(e.sync_kind())
            }
        },
        Err(e) if is_docsrs_fallback_eligible(&e) => {
            warn!(
                "  ⚠ docs.rs unavailable for {crate_name}@{version}: {e}; trying GitHub fallback"
            );
            sync_one_crate_from_github(
                rust_output_dir,
                github_fetcher,
                crate_name,
                crate_doc,
                version,
                max_file_size_kb,
                Some("github_fallback"),
            )
            .await
        }
        Err(e) => {
            warn!("  ✗ docs.rs fetch failed for {crate_name}@{version}: {e}");
            SyncOutcome::Error(e.sync_kind())
        }
    }
}

async fn sync_one_crate_from_github(
    rust_output_dir: PathBuf,
    fetcher: Arc<GitHubFetcher>,
    crate_name: String,
    crate_doc: crate::config::CrateDoc,
    version: String,
    max_file_size_kb: usize,
    source_kind_override: Option<&'static str>,
) -> SyncOutcome {
    let Some(repo) = crate_doc.github_repo().map(str::to_string) else {
        warn!("Crate '{crate_name}' has no GitHub repo in config");
        if source_kind_override.is_some() {
            return SyncOutcome::Error(SyncErrorKind::Other);
        }
        return SyncOutcome::Skipped;
    };

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

    let requests = build_requests(crate_doc.subpath.as_deref(), crate_doc.effective_files());
    let results = fetcher
        .fetch_files(&repo, &resolved.git_ref, &requests)
        .await;

    let fetched_files = collect_fetched_files(results, &crate_name, &version);
    if fetched_files.files.is_empty() {
        warn!("  ✗ no files fetched for {crate_name}@{version}");
        return SyncOutcome::Error(SyncErrorKind::NotFound);
    }

    let source_kind = source_kind_override.unwrap_or("github");
    let save_ctx = storage::SaveContext {
        repo: &repo,
        resolved: &resolved,
        max_file_size_kb,
        source_kind,
        artifact_path: None,
        docsrs_input_url: None,
        upstream_latest_version: Some(&version),
        truncated: None,
    };

    let save_req = storage::SaveRequest {
        crate_name: &crate_name,
        version: &version,
        fetched_files: &fetched_files.files,
        crate_config: &crate_doc,
    };

    match storage::save_crate_files(&rust_output_dir, &save_ctx, save_req) {
        Ok(saved) => SyncOutcome::Synced(saved),
        Err(e) => SyncOutcome::Error(e.sync_kind()),
    }
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

    if !force && storage::is_cached(&rust_output_dir, &crate_name, &version, &crate_doc) {
        info!("  ⏭ {crate_name}@{version}: cached, skipping");
        let cached = storage::read_cached_info(&rust_output_dir, &crate_name, &version, &crate_doc);
        return SyncOutcome::Cached(cached);
    }

    info!("Syncing {crate_name}@{version}...");

    sync_one_crate_hybrid(
        rust_output_dir,
        fetcher,
        crate_name,
        crate_doc,
        version,
        max_file_size_kb,
    )
    .await
}

async fn sync_one_crate_hybrid(
    rust_output_dir: PathBuf,
    github_fetcher: Arc<GitHubFetcher>,
    crate_name: String,
    crate_doc: crate::config::CrateDoc,
    version: String,
    max_file_size_kb: usize,
) -> SyncOutcome {
    // 1. Try fetching from docs.rs first
    let latest_fetcher = LatestDocsFetcher::new();
    let docsrs_readme = match latest_fetcher
        .fetch_api_markdown(&crate_name, &version, max_file_size_kb)
        .await 
    {
        Ok(artifact) => {
            info!("  ✓ {crate_name}@{version}: description fetched from docs.rs");
            Some(artifact)
        }
        Err(e) => {
            warn!("  ⚠️ docs.rs fetch failed for {crate_name}@{version}: {e}; will use GitHub README");
            None
        }
    };

    // 2. Resolve GitHub Ref
    let Some(repo) = crate_doc.github_repo().map(str::to_string) else {
        warn!("Crate '{crate_name}' has no GitHub repo in config");
        // Fallback: if we have docs.rs content, save it and consider it synced.
        if let Some(art) = docsrs_readme {
             match storage::save_latest_api_markdown(
                &rust_output_dir,
                &crate_name,
                &version,
                &art.markdown,
                &art.docsrs_input_url,
                art.truncated,
                &crate_doc,
            ) {
                Ok(saved) => return SyncOutcome::Synced(saved),
                Err(e) => return SyncOutcome::Error(e.sync_kind()),
            }
        }
        return SyncOutcome::Skipped;
    };

    let resolved = match github_fetcher
        .resolve_ref(&repo, &crate_name, version.as_str())
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("  ✗ failed to resolve ref for {crate_name}@{version}: {e}");
            return SyncOutcome::Error(e.sync_kind());
        }
    };

    // 3. Build Requests
    let mut requests = build_requests(crate_doc.subpath.as_deref(), crate_doc.effective_files());
    
    // If we have docs.rs README, remove README from GitHub requests
    if docsrs_readme.is_some() {
        requests.retain(|r| !is_readme_request(&r.original_path));
    }

    // 4. Fetch from GitHub
    let results = github_fetcher
        .fetch_files(&repo, &resolved.git_ref, &requests)
        .await;

    let mut fetch_collection = collect_fetched_files(results, &crate_name, &version);

    // 5. Inject docs.rs README if available
    if let Some(art) = docsrs_readme {
        fetch_collection.files.push(FetchedFile {
            path: "README.md".to_string(),
            source_url: art.docsrs_input_url.clone(), // Point to docs.rs as source
            content: art.markdown,
        });
    }

    if fetch_collection.files.is_empty() {
        warn!("  ✗ no files fetched for {crate_name}@{version}");
        return SyncOutcome::Error(SyncErrorKind::NotFound);
    }

    let save_ctx = storage::SaveContext {
        repo: &repo,
        resolved: &resolved,
        max_file_size_kb,
        source_kind: "hybrid_docsrs_github",
        artifact_path: None,
        docsrs_input_url: None, // We embedded it in the file source_url
        upstream_latest_version: Some(&version),
        truncated: None,
    };

    let save_req = storage::SaveRequest {
        crate_name: &crate_name,
        version: &version,
        fetched_files: &fetch_collection.files,
        crate_config: &crate_doc,
    };

    match storage::save_crate_files(&rust_output_dir, &save_ctx, save_req) {
        Ok(saved) => SyncOutcome::Synced(saved),
        Err(e) => SyncOutcome::Error(e.sync_kind()),
    }
}

fn is_readme_request(path: &str) -> bool {
    path.eq_ignore_ascii_case("README.md")
}

// Helper to identify likely README requests
fn is_readme_request(path: &str) -> bool {
    path.eq_ignore_ascii_case("README.md")
}

async fn sync_one_crate_from_github(
    rust_output_dir: PathBuf,
    fetcher: Arc<GitHubFetcher>,
    crate_name: String,
    crate_doc: crate::config::CrateDoc,
    version: String,
    max_file_size_kb: usize,
    source_kind_override: Option<&'static str>,
) -> SyncOutcome {
    let Some(repo) = crate_doc.github_repo().map(str::to_string) else {
        warn!("Crate '{crate_name}' has no GitHub repo in config");
        if source_kind_override.is_some() {
            return SyncOutcome::Error(SyncErrorKind::Other);
        }
        return SyncOutcome::Skipped;
    };

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

    let requests = build_requests(crate_doc.subpath.as_deref(), crate_doc.effective_files());
    let results = fetcher
        .fetch_files(&repo, &resolved.git_ref, &requests)
        .await;

    let fetched_files = collect_fetched_files(results, &crate_name, &version);
    if fetched_files.files.is_empty() {
        warn!("  ✗ no files fetched for {crate_name}@{version}");
        return SyncOutcome::Error(SyncErrorKind::NotFound);
    }

    let source_kind = source_kind_override.unwrap_or("github");
    let save_ctx = storage::SaveContext {
        repo: &repo,
        resolved: &resolved,
        max_file_size_kb,
        source_kind,
        artifact_path: None,
        docsrs_input_url: None,
        upstream_latest_version: Some(&version),
        truncated: None,
    };

    let save_req = storage::SaveRequest {
        crate_name: &crate_name,
        version: &version,
        fetched_files: &fetched_files.files,
        crate_config: &crate_doc,
    };

    match storage::save_crate_files(&rust_output_dir, &save_ctx, save_req) {
        Ok(saved) => SyncOutcome::Synced(saved),
        Err(e) => SyncOutcome::Error(e.sync_kind()),
    }
}

struct FetchCollection {
    files: Vec<FetchedFile>,
    _non_optional_errors: usize,
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
        _non_optional_errors: non_optional_errors,
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

async fn run_orchestrated_sync<F, Fut>(
    config: &Config,
    jobs: Vec<(String, crate::config::CrateDoc)>,
    worker: F,
) -> Vec<SyncOutcome>
where
    F: Fn(String, crate::config::CrateDoc) -> Fut,
    Fut: std::future::Future<Output = SyncOutcome> + Send + 'static,
{
    let concurrency = config.settings.sync_concurrency;
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut join_set = tokio::task::JoinSet::new();

    for (name, doc) in jobs {
        let semaphore = Arc::clone(&semaphore);
        let fut = worker(name, doc);
        join_set.spawn(async move {
            let _permit = semaphore.acquire_owned().await.expect("semaphore closed");
            fut.await
        });
    }

    let mut outcomes = Vec::new();
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(outcome) => outcomes.push(outcome),
            Err(e) => {
                warn!("sync worker panicked: {e}");
                outcomes.push(SyncOutcome::Error(SyncErrorKind::Other));
            }
        }
    }
    outcomes
}

fn resolve_sync_mode(mode_override: Option<SyncModeArg>, configured_mode: SyncMode) -> SyncMode {
    mode_override
        .map(SyncModeArg::to_sync_mode)
        .unwrap_or(configured_mode)
}



const fn should_emit_plain_check_errors(format: OutputFormat, github_actions: bool) -> bool {
    !github_actions && matches!(format, OutputFormat::Table)
}

fn emit_check_failures_for_ci(format: OutputFormat, statuses: &[crate::status::CrateStatus]) {
    let github_actions = std::env::var("GITHUB_ACTIONS")
        .ok()
        .is_some_and(|v| v == "true");

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

async fn run_status(
    config_path: &Path,
    mode_override: Option<SyncModeArg>,
    format: OutputFormat,
) -> Result<()> {
    let config = Config::load(config_path)?;
    info!("Loaded config from {}", config_path.display());
    let rust_dir = storage::rust_output_dir(&config.settings.output_dir);

    let sync_mode = resolve_sync_mode(mode_override, config.settings.sync_mode);

    let statuses = match sync_mode {
        SyncMode::Lockfile | SyncMode::Hybrid => {
            let rust_versions =
                resolver::resolve_cargo_versions(PathBuf::from("Cargo.lock").as_path())?;
            collect_status(&config, &rust_versions, &rust_dir).await
        }
        SyncMode::LatestDocs => {
            let fetcher = LatestDocsFetcher::new(reqwest::Client::new());
            collect_status_latest(&config, &rust_dir, Some(&fetcher)).await
        }
    };

    print_statuses(format, &statuses)
}

async fn run_check(
    config_path: &Path,
    mode_override: Option<SyncModeArg>,
    format: OutputFormat,
) -> Result<()> {
    let config = Config::load(config_path)?;
    info!("Loaded config from {}", config_path.display());
    let rust_dir = storage::rust_output_dir(&config.settings.output_dir);

    let sync_mode = resolve_sync_mode(mode_override, config.settings.sync_mode);

    let statuses = match sync_mode {
        SyncMode::Lockfile | SyncMode::Hybrid => {
            let rust_versions =
                resolver::resolve_cargo_versions(PathBuf::from("Cargo.lock").as_path())?;
            collect_status(&config, &rust_versions, &rust_dir).await
        }
        SyncMode::LatestDocs => {
            let fetcher = LatestDocsFetcher::new(reqwest::Client::new());
            collect_status_latest(&config, &rust_dir, Some(&fetcher)).await
        }
    };
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
        build_requests, collect_fetched_files, is_latest_cache_fresh, resolve_sync_mode,
        should_emit_plain_check_errors, OutputFormat, SyncMode, SyncModeArg,
    };
    use crate::error::AiDocsError;
    use crate::fetcher::github::FetchedFile;
    use clap::{CommandFactory, Parser};

    #[test]
    fn latest_cache_freshness_respects_ttl_hours() {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert!(crate::utils::is_latest_cache_fresh(&today, 24));

        assert!(!crate::utils::is_latest_cache_fresh("1970-01-01", 24));
        assert!(!crate::utils::is_latest_cache_fresh("invalid-date", 24));
    }

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
    fn resolve_sync_mode_prefers_cli_override() {
        let mode = resolve_sync_mode(Some(SyncModeArg::LatestDocs), SyncMode::Lockfile);
        assert_eq!(mode, SyncMode::LatestDocs);
    }

    #[test]
    fn resolve_sync_mode_uses_settings_when_cli_not_set() {
        let mode = resolve_sync_mode(None, SyncMode::Lockfile);
        assert_eq!(mode, SyncMode::Lockfile);
    }

    #[test]
    fn sync_mode_defaults_to_lockfile_when_flag_not_provided() {
        let cli = super::Cli::parse_from(["ai-fdocs", "sync"]);
        let super::Commands::Sync { mode, .. } = cli.command else {
            panic!("expected sync command");
        };

        assert!(mode.is_none(), "sync --mode should be optional");
        let resolved = resolve_sync_mode(mode, SyncMode::Lockfile);
        assert_eq!(resolved, SyncMode::Lockfile);
    }

    #[test]
    fn status_mode_defaults_to_none_when_flag_not_provided() {
        let cli = super::Cli::parse_from(["ai-fdocs", "status"]);
        let super::Commands::Status { mode, .. } = cli.command else {
            panic!("expected status command");
        };

        assert!(mode.is_none(), "status --mode should be optional");
    }

    #[test]
    fn check_mode_parses_latest_docs_override() {
        let cli = super::Cli::parse_from(["ai-fdocs", "check", "--mode", "latest-docs"]);
        let super::Commands::Check { mode, .. } = cli.command else {
            panic!("expected check command");
        };

        assert_eq!(mode, Some(SyncModeArg::LatestDocs));
    }

    #[test]
    fn cli_subcommands_have_consistent_help_and_config_flag() {
        let mut command = super::Cli::command();
        command.build();

        for sub in ["sync", "status", "check", "init"] {
            let sub_cmd = command
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
