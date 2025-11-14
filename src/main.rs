mod browser_auth;
mod cli;
mod config;
mod config_cmd;
mod monitor;
mod notifications;
mod retry;
mod service;
mod setup;
mod state;
mod status;
mod storage;
mod usage;

use clap::Parser;
use cli::{Cli, Commands, ConfigAction};
use tracing_subscriber;

fn main() -> anyhow::Result<()> {
    // Parse CLI arguments first to get verbose flag
    let cli = Cli::parse();

    // Initialize logging based on verbose flag
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::DEBUG.into()),
            )
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::ERROR.into()),
            )
            .init();
    }

    // Route to appropriate command handler
    match cli.command {
        Commands::Setup => {
            setup::run_setup(cli.verbose)?;
        }
        Commands::Run => {
            monitor::run_monitor(true)?; // true = foreground mode
        }
        Commands::Usage => {
            usage::run_usage(cli.verbose)?;
        }
        Commands::Status => {
            status::run_status(cli.verbose)?;
        }
        Commands::Config { action } => {
            match action {
                ConfigAction::List => {
                    config_cmd::handle_config_list()?;
                }
                ConfigAction::Get { key } => {
                    config_cmd::handle_config_get(&key)?;
                }
                ConfigAction::Set { key, value } => {
                    config_cmd::handle_config_set(&key, &value)?;
                }
            }
        }
        Commands::SetupService => {
            service::install_service()?;
        }
        Commands::Start => {
            service::start_service()?;
        }
        Commands::Stop => {
            service::stop_service()?;
        }
        Commands::UninstallService => {
            service::uninstall_service()?;
        }
    }

    Ok(())
}
