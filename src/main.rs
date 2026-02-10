mod config;
mod error;
mod fetcher;
mod index;
mod processor;
mod resolver;
mod status;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use crate::config::{Config, SourceType};
use crate::error::Result;
use crate::fetcher::GitHubFetcher;
use crate::resolver::LockResolver;

#[derive(Parser)]
#[command(name = "cargo-ai-fdocs")]
#[command(bin_name = "cargo")]
enum CargoCli {
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
    Sync {
        #[arg(short, long, default_value = "ai-docs.toml")]
        config: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Show which docs are outdated vs lock files
    Status {
        #[arg(short, long, default_value = "ai-docs.toml")]
        config: PathBuf,
    },
    Status,
    Check,
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
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args: Vec<String> = std::env::args()
        .enumerate()
        .filter(|(i, arg)| !(*i == 1 && arg == "ai-docs"))
        .map(|(_, arg)| arg)
        .collect();

async fn run() -> Result<()> {
    let CargoCli::AiFdocs(cli) = CargoCli::parse();

    match cli.command {
        Commands::Sync { config, force } => {
            if let Err(e) = run_sync(&config, force).await {
                error!("Sync failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Status { config } => {
            if let Err(e) = run_status(&config).await {
                error!("Status check failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

async fn run_sync(config_path: &PathBuf, force: bool) -> error::Result<()> {
    let config = Config::load(config_path)?;
    info!("Loaded config from {}", config_path.display());

    let cargo_lock_path = PathBuf::from("Cargo.lock");
    let rust_versions = if cargo_lock_path.exists() {
        resolver::resolve_cargo_versions(&cargo_lock_path)?
    } else {
        warn!("Cargo.lock not found, skipping Rust dependencies");
        std::collections::HashMap::new()
    };

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

                if let Some(paths) = &crate_cfg.files {
                    info!("  Explicit files configured: {}", paths.len());
                    for path in paths {
                        match fetcher
                            .fetch_file(&github_source.repo, &resolved.git_ref, path)
                            .await?
                        {
                            Some(content) => {
                                info!("  ✅ '{}' fetched ({} bytes)", path, content.len())
                            }
                            None => warn!("  ❌ '{}' not found", path),
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
        Commands::Status => {
            run_status()?;
        }
        Commands::Check => {
            run_status()?;
        }
    }

    Ok(())
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

fn print_config_example() {
    eprintln!("ai-fdocs.toml not found. Create one in your project root.");
    eprintln!();
    eprintln!("Example:");
    eprintln!("[crates.axum]");
    eprintln!("sources = [{{ type = \"github\", repo = \"tokio-rs/axum\" }}]");
    eprintln!();
    eprintln!("[crates.serde]");
    eprintln!("sources = [{{ type = \"github\", repo = \"serde-rs/serde\" }}]");
    eprintln!("ai_notes = \"Use derive macros for serialization.\"");
}
