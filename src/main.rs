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

use crate::config::{Config, Source};
use crate::error::Result;
use crate::fetcher::github::GitHubFetcher;
use crate::status::{collect_status, print_status_table, DocsStatus};

#[derive(Parser)]
#[command(name = "cargo-ai-fdocs")]
#[command(bin_name = "cargo")]
enum CargoCli {
    #[command(name = "ai-docs")]
    AiDocs(Cli),
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
        #[arg(short, long, default_value = "ai-docs.toml")]
        config: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Show which docs are outdated vs lock file
    Status {
        #[arg(short, long, default_value = "ai-docs.toml")]
        config: PathBuf,
    },
    /// Exit with 1 if docs are missing/outdated/corrupted
    Check {
        #[arg(short, long, default_value = "ai-docs.toml")]
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
        .filter(|(i, arg)| !(*i == 1 && (arg == "ai-docs" || arg == "ai-fdocs")))
        .map(|(_, arg)| arg)
        .collect();

    let parse = CargoCli::try_parse_from(args).map(|parsed| match parsed {
        CargoCli::AiDocs(cli) | CargoCli::AiFdocs(cli) => cli,
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

                let Some(version) = locked_versions.get(name) else {
                    warn!("Crate '{name}' not found in Cargo.lock. Skipping.");
                    continue;
                };
                info!("  Locked version: {version}");

                let Some(github_source) = crate_cfg
                    .sources
                    .iter()
                    .find(|source| source.source_type == SourceType::Github)
                else {
                    warn!("  ❌ no source with type='github' configured. Skipping.");
                    continue;
                };

                let resolved = fetcher
                    .resolve_ref(&github_source.repo, name, version)
                    .await?;
                if resolved.is_fallback {
                    warn!("  ⚠ Fallback to branch: {}", resolved.git_ref);
                } else {
                    info!("  Tag found: {}", resolved.git_ref);
                }

        for source in &crate_doc.sources {
            match source {
                Source::GitHub { repo, files } => {
                    let resolved = match fetcher.resolve_ref(repo, version.as_str()).await {
                        Ok(r) => r,
                        Err(e) => {
                            warn!("  ✗ failed to resolve ref: {e}");
                            stats.errors += 1;
                            continue;
                        }
                    }
                } else {
                    match fetcher
                        .fetch_file(&github_source.repo, &resolved.git_ref, "README.md")
                        .await?
                    {
                        Some(content) => info!("  ✅ README.md fetched ({} bytes)", content.len()),
                        None => warn!("  ❌ README.md not found at README.md"),
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
                    crate_saved = Some(saved);
                    break;
                }
                Source::DocsRs => {
                    info!("  ⏭ docs.rs source skipped (not implemented in MVP)");
                }
            }
        }

        if let Some(saved) = crate_saved {
            saved_crates.push(saved);
            stats.synced += 1;
        } else {
            stats.skipped += 1;
        }
    }

    index::generate_index(&rust_output_dir, &saved_crates)?;

    info!(
        "✅ Sync complete: {} synced, {} cached, {} skipped, {} errors",
        stats.synced, stats.cached, stats.skipped, stats.errors
    );

    Ok(())
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
            "Documentation is outdated or missing. Run: cargo ai-docs sync".to_string(),
        ));
    }

    info!("All configured crate docs are up to date.");
    Ok(())
}
