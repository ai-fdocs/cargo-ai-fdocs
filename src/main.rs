mod config;
mod error;
// mod resolver; // Stage 2
// mod fetcher;  // Stage 2
// mod storage;  // Stage 3

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::error::Result;

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
    /// Sync documentation based on lockfile
    Sync {
        /// Force re-download ignoring cache
        #[arg(short, long)]
        force: bool,
    },
    /// Show current documentation status
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

    let config_path = PathBuf::from("ai-fdocs.toml");

    match cli.command {
        Commands::Sync { force } => {
            info!("Starting sync... (force={force})");

            match Config::load(&config_path) {
                Ok(cfg) => {
                    info!("Config loaded successfully.");
                    info!("Output dir: {:?}", cfg.settings.output_dir);
                    info!("Crates defined: {}", cfg.crates.len());
                    for (name, c) in cfg.crates {
                        info!("  - {}: {} (subpath: {:?})", name, c.repo, c.subpath);
                    }
                }
                Err(error::AiDocsError::ConfigNotFound(_)) => {
                    warn!("Config not found. Please create ai-fdocs.toml");
                }
                Err(e) => return Err(e),
            }
        }
        Commands::Status => {
            info!("Checking status...");
            println!("Crate            Lock Version    Docs Status");
            println!("─────────────────────────────────────────────");
            println!("(Not implemented yet)");
        }
    }

    Ok(())
}
