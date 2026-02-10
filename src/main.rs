mod config;
mod error;
mod fetcher;
mod resolver;
mod status;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::error::Result;
use crate::fetcher::GitHubFetcher;
use crate::resolver::LockResolver;
use crate::status::{exit_code, is_healthy, SyncStatus};

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
    Check,
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
        Commands::Sync { force } => {
            info!("Starting sync... (force={force})");

            let config_path = PathBuf::from("ai-fdocs.toml");
            let config = match Config::load(&config_path) {
                Ok(config) => config,
                Err(crate::error::AiDocsError::ConfigNotFound(_)) => {
                    print_config_example();
                    return Ok(());
                }
                Err(err) => return Err(err),
            };
            info!(
                "Config loaded. Processing {} crates...",
                config.crates.len()
            );
            info!(
                "Settings: output_dir='{}', max_file_size_kb={}, prune={}",
                config.settings.output_dir.display(),
                config.settings.max_file_size_kb,
                config.settings.prune
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
                if !crate_cfg.ai_notes.is_empty() {
                    info!("  AI notes configured ({} chars)", crate_cfg.ai_notes.len());
                }

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

                if let Some(paths) = &crate_cfg.files {
                    info!("  Explicit files configured: {}", paths.len());
                    for path in paths {
                        match fetcher
                            .fetch_file(&crate_cfg.repo, &resolved.git_ref, path)
                            .await?
                        {
                            Some(content) => {
                                info!("  ✅ '{}' fetched ({} bytes)", path, content.len())
                            }
                            None => warn!("  ❌ '{}' not found", path),
                        }
                    }
                } else {
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
        }
        Commands::Status => {
            let statuses = collect_statuses();
            println!("Current statuses: {:?}", statuses);
            println!("Healthy: {}", is_healthy(&statuses));
        }
        Commands::Check => {
            let statuses = collect_statuses();
            std::process::exit(exit_code(&statuses));
        }
    }

    Ok(())
}

fn collect_statuses() -> Vec<SyncStatus> {
    Vec::new()
}

fn print_config_example() {
    eprintln!("ai-fdocs.toml not found. Create one in your project root.");
    eprintln!();
    eprintln!("Example:");
    eprintln!("[crates.axum]");
    eprintln!("repo = \"tokio-rs/axum\"");
    eprintln!();
    eprintln!("[crates.serde]");
    eprintln!("repo = \"serde-rs/serde\"");
    eprintln!("ai_notes = \"Use derive macros for serialization.\"");
}

mod status {
    use crate::error::Result;

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

    pub fn collect_status() -> Result<Vec<Entry>> {
        Ok(Vec::new())
    }

    pub fn is_healthy(statuses: &[Entry]) -> bool {
        statuses
            .iter()
            .all(|entry| matches!(entry.state, State::Synced | State::SyncedFallback))
    }

    pub fn print_status(statuses: &[Entry]) {
        if statuses.is_empty() {
            println!("No crates to inspect. Run `cargo ai-fdocs sync` first.");
            return;
        }

        for entry in statuses {
            println!("{}: {}", entry.crate_name, state_label(&entry.state));
        }
    }

    pub fn print_check(statuses: &[Entry]) {
        let unhealthy: Vec<&Entry> = statuses
            .iter()
            .filter(|entry| {
                matches!(
                    entry.state,
                    State::Missing | State::Outdated | State::Corrupted
                )
            })
            .collect();

        if unhealthy.is_empty() {
            println!("ok ({} crates)", statuses.len());
            return;
        }

        let names = unhealthy
            .iter()
            .map(|entry| entry.crate_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!("stale: {} crate(s): {}", unhealthy.len(), names);
    }

    fn state_label(state: &State) -> &'static str {
        match state {
            State::Synced => "synced",
            State::SyncedFallback => "synced_fallback",
            State::Missing => "missing",
            State::Outdated => "outdated",
            State::Corrupted => "corrupted",
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{is_healthy, Entry, State};

        #[test]
        fn healthy_when_only_synced_or_fallback() {
            let statuses = vec![
                Entry {
                    crate_name: "axum".to_string(),
                    state: State::Synced,
                },
                Entry {
                    crate_name: "serde".to_string(),
                    state: State::SyncedFallback,
                },
            ];

            assert!(is_healthy(&statuses));
        }

        #[test]
        fn unhealthy_when_missing_present() {
            let statuses = vec![Entry {
                crate_name: "axum".to_string(),
                state: State::Missing,
            }];

            assert!(!is_healthy(&statuses));
        }

        #[test]
        fn unhealthy_when_outdated_present() {
            let statuses = vec![Entry {
                crate_name: "axum".to_string(),
                state: State::Outdated,
            }];

            assert!(!is_healthy(&statuses));
        }

        #[test]
        fn unhealthy_when_corrupted_present() {
            let statuses = vec![Entry {
                crate_name: "axum".to_string(),
                state: State::Corrupted,
            }];

            assert!(!is_healthy(&statuses));
        }
    }
}
