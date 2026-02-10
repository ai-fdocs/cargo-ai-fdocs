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

use crate::config::{Config, Source};
use crate::fetcher::github::GitHubFetcher;

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

        let mut crate_saved: Option<storage::SavedCrate> = None;

        for source in &crate_doc.sources {
            match source {
                Source::GitHub { repo, files } => {
                    let resolved = match fetcher.resolve_ref(repo, &version).await {
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

                    let results = fetcher.fetch_files(repo, &resolved.git_ref, files).await;
                    let fetched_files: Vec<_> = results
                        .into_iter()
                        .filter_map(|r| match r {
                            Ok(file) => Some(file),
                            Err(e) => {
                                warn!("  ✗ {e}");
                                None
                            }
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
                    crate_saved = Some(saved);
                    break;
                }
                Source::DocsRs => {
                    info!("  ⏭ docs.rs source skipped (not implemented in MVP)");
                }
            }
        }
        Commands::Status => {
            let config_path = PathBuf::from("ai-fdocs.toml");
            let config = match Config::load(&config_path) {
                Ok(config) => config,
                Err(crate::error::AiDocsError::ConfigNotFound(_)) => {
                    print_config_example();
                    return Ok(());
                }
                Err(err) => return Err(err),
            };

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

async fn run_status(config_path: &PathBuf) -> error::Result<()> {
    let config = Config::load(config_path)?;

    let cargo_lock_path = PathBuf::from("Cargo.lock");
    let rust_versions = if cargo_lock_path.exists() {
        resolver::resolve_cargo_versions(&cargo_lock_path)?
    } else {
        std::collections::HashMap::new()
    };

    let rust_dir = storage::rust_output_dir(&config.settings.output_dir);

    println!("Dependency Status:");
    println!("{:-<60}", "");

    for (crate_name, _) in &config.crates {
        let lock_version = rust_versions
            .get(crate_name.as_str())
            .cloned()
            .unwrap_or_else(|| "???".to_string());

        let crate_dir = rust_dir.join(format!("{crate_name}@{lock_version}"));

        let status = if crate_dir.exists() {
            "✅ OK".to_string()
        } else {
            let existing = find_existing_version(&rust_dir, crate_name);
            match existing {
                Some(old_ver) => format!("⚠️  OUTDATED ({old_ver} → {lock_version})"),
                None => "❌ MISSING".to_string(),
            }
        };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum State {
        Synced,
        SyncedFallback,
        Missing,
        Outdated,
        Corrupted,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Entry {
        pub crate_name: String,
        pub state: State,
    }

fn find_existing_version(ecosystem_dir: &std::path::Path, crate_name: &str) -> Option<String> {
    let prefix = format!("{crate_name}@");
    if let Ok(entries) = std::fs::read_dir(ecosystem_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                return Some(name.trim_start_matches(&prefix).to_string());
            }
        }
    }
    None
}
