mod config;
mod error;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::error::Result;

#[derive(Debug, Parser)]
#[command(name = "cargo-ai-fdocs")]
#[command(about = "Version-locked docs for AI coding assistants")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Sync docs for configured crates
    Sync {
        /// Path to config file
        #[arg(short, long, default_value = "ai-fdocs.toml")]
        config: PathBuf,

        /// Ignore cache and force refetch
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Show current docs status
    Status {
        /// Path to config file
        #[arg(short, long, default_value = "ai-fdocs.toml")]
        config: PathBuf,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = normalize_args_for_cargo_subcommand();
    let cli = Cli::parse_from(args);

    match cli.command {
        Commands::Sync { config, force } => {
            let cfg = Config::load(&config)?;
            println!("Loaded config: {}", config.display());
            println!("Settings: {:?}", cfg.settings);
            println!("Crates count: {}", cfg.crates.len());
            println!("force = {force}");

            todo!("sync logic will be implemented in next stage");
        }
        Commands::Status { config } => {
            let cfg = Config::load(&config)?;
            println!("Loaded config: {}", config.display());
            println!("Settings: {:?}", cfg.settings);
            println!("Crates count: {}", cfg.crates.len());

            todo!("status logic will be implemented in next stage");
        }
    }
}

fn normalize_args_for_cargo_subcommand() -> Vec<String> {
    let raw: Vec<String> = std::env::args().collect();

    if raw.len() > 1 && raw[1] == "ai-fdocs" {
        let mut normalized = Vec::with_capacity(raw.len() - 1);
        normalized.push(raw[0].clone());
        normalized.extend(raw.into_iter().skip(2));
        normalized
    } else {
        raw
    }
}
