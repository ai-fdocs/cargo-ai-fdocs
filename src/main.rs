mod config;
mod error;
mod fetcher;
mod resolver;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use crate::config::Config;
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
        #[arg(short, long)]
        force: bool,
    },
    Status,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    if let Err(e) = run().await {
        error!("Fatal error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let CargoCli::AiFdocs(cli) = CargoCli::parse();

    match cli.command {
        Commands::Sync { .. } => {
            let config_path = PathBuf::from("ai-fdocs.toml");
            let config = Config::load(&config_path)?;
            info!(
                "Config loaded. Processing {} crates...",
                config.crates.len()
            );

            let lock_path = PathBuf::from("Cargo.lock");
            let locked_versions = LockResolver::resolve(&lock_path)?;
            info!(
                "Cargo.lock parsed. Found {} packages.",
                locked_versions.len()
            );

            let fetcher = GitHubFetcher::new()?;
            if fetcher.token_present {
                info!("GitHub token detected.");
            }

            for (name, crate_cfg) in &config.crates {
                info!("Processing crate: {name}");

                let Some(version) = locked_versions.get(name) else {
                    warn!("Crate '{name}' not found in Cargo.lock. Skipping.");
                    continue;
                };
                info!("  Locked version: {version}");

                let resolved = fetcher.resolve_ref(&crate_cfg.repo, name, version).await?;
                if resolved.is_fallback {
                    warn!("  ⚠ Fallback to branch: {}", resolved.git_ref);
                } else {
                    info!("  Tag found: {}", resolved.git_ref);
                }

                let readme_path = if let Some(sub) = &crate_cfg.subpath {
                    format!("{sub}/README.md")
                } else {
                    "README.md".to_string()
                };

                match fetcher
                    .fetch_file(&crate_cfg.repo, &resolved.git_ref, &readme_path)
                    .await?
                {
                    Some(content) => info!("  ✅ README.md fetched ({} bytes)", content.len()),
                    None => warn!("  ❌ README.md not found at {readme_path}"),
                }
            }
        }
        Commands::Status => {
            println!("(Status command implementation pending Stage 4)");
        }
    }

    Ok(())
}
