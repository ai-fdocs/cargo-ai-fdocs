mod config;
mod error;
#[path = "fetcher/mod.rs"]
mod fetcher;
mod index;
mod processor;
mod resolver;
mod status;
mod storage;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::error::Result;
use crate::fetcher::github::{FileRequest, GitHubFetcher};
use crate::status::{collect_status, print_status_table, DocsStatus};

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
        #[arg(short, long, default_value = "ai-fdocs.toml")]
        config: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    Status {
        #[arg(short, long, default_value = "ai-fdocs.toml")]
        config: PathBuf,
    },
    Check {
        #[arg(short, long, default_value = "ai-fdocs.toml")]
        config: PathBuf,
    },
}

#[derive(Default)]
struct SyncStats {
    synced: usize,
    cached: usize,
    skipped: usize,
    errors: usize,
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
        Commands::Status { config } => run_status(&config),
        Commands::Check { config } => run_check(&config),
    }
}

async fn run_sync(config_path: &PathBuf, force: bool) -> Result<()> {
    let config = Config::load(config_path)?;
    info!("Loaded config from {}", config_path.display());

    let cargo_lock_path = PathBuf::from("Cargo.lock");
    let rust_versions = resolver::resolve_cargo_versions(&cargo_lock_path)?;

    let rust_output_dir = storage::rust_output_dir(&config.settings.output_dir);
    if config.settings.prune {
        storage::prune(&rust_output_dir, &config, &rust_versions)?;
    }

    let fetcher = GitHubFetcher::new();
    let mut saved_crates = Vec::new();
    let mut stats = SyncStats::default();

    for (crate_name, crate_doc) in &config.crates {
        let Some(version) = rust_versions.get(crate_name.as_str()).cloned() else {
            warn!("Crate '{crate_name}' not found in Cargo.lock, skipping");
            stats.skipped += 1;
            continue;
        };

        let Some(repo) = crate_doc.github_repo() else {
            warn!("Crate '{crate_name}' has no GitHub repo in config, skipping");
            stats.skipped += 1;
            continue;
        };

        if !force && storage::is_cached(&rust_output_dir, crate_name, &version) {
            info!("  ⏭ {crate_name}@{version}: cached, skipping");
            if let Some(saved) =
                storage::read_cached_info(&rust_output_dir, crate_name, &version, crate_doc)
            {
                saved_crates.push(saved);
            }
            stats.cached += 1;
            continue;
        }

        info!("Syncing {crate_name}@{version}...");

        let resolved = match fetcher
            .resolve_ref(repo, crate_name, version.as_str())
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("  ✗ failed to resolve ref: {e}");
                stats.errors += 1;
                continue;
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
            .fetch_files(repo, &resolved.git_ref, &requests)
            .await;

        let fetched_files: Vec<_> = results
            .into_iter()
            .filter_map(|r| match r {
                Ok(file) => Some(file),
                Err(e) => match e {
                    crate::error::AiDocsError::OptionalFileNotFound(_) => None,
                    other => {
                        warn!("  ✗ {other}");
                        None
                    }
                },
            })
            .collect();

        if fetched_files.is_empty() {
            warn!("  ✗ no files fetched for {crate_name}@{version}");
            stats.errors += 1;
            continue;
        }

        let saved = storage::save_crate_files(
            &rust_output_dir,
            crate_name,
            &version,
            repo,
            &resolved,
            &fetched_files,
            crate_doc,
            config.settings.max_file_size_kb,
        )?;
        saved_crates.push(saved);
        stats.synced += 1;
    }

    index::generate_index(&rust_output_dir, &saved_crates)?;

    info!(
        "✅ Sync complete: {} synced, {} cached, {} skipped, {} errors",
        stats.synced, stats.cached, stats.skipped, stats.errors
    );

    Ok(())
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

fn run_status(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;
    let rust_versions = resolver::resolve_cargo_versions(PathBuf::from("Cargo.lock").as_path())?;

    let rust_dir = storage::rust_output_dir(&config.settings.output_dir);
    let statuses = collect_status(&config, &rust_versions, &rust_dir);
    print_status_table(&statuses);

    Ok(())
}

fn run_check(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;
    let rust_versions = resolver::resolve_cargo_versions(PathBuf::from("Cargo.lock").as_path())?;
    let rust_dir = storage::rust_output_dir(&config.settings.output_dir);

    let statuses = collect_status(&config, &rust_versions, &rust_dir);
    let failing = statuses
        .iter()
        .any(|s| !matches!(s.status, DocsStatus::Synced | DocsStatus::SyncedFallback));

    if failing {
        print_status_table(&statuses);
        return Err(error::AiDocsError::Other(
            "Documentation is outdated or missing. Run: cargo ai-fdocs sync".to_string(),
        ));
    }

    info!("All configured crate docs are up to date.");
    Ok(())
}
