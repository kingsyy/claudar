mod browser_auth;
mod cli;
mod config;
mod config_cmd;
mod monitor;
mod notification_trait;
mod notifications;
mod pace;
mod retry;
mod service;
mod setup;
mod state;
mod status;
mod storage;
mod time_format;
mod usage;

use clap::Parser;
use cli::{Cli, Commands, ConfigAction, InstancesAction};
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
        Commands::Setup { instance } => {
            setup::run_setup(cli.verbose, instance)?;
        }
        Commands::Run => {
            monitor::run_monitor(true)?; // true = foreground mode
        }
        Commands::Usage { instance } => {
            usage::run_usage(cli.verbose, instance)?;
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
        Commands::Instances { action } => {
            match action {
                InstancesAction::List => {
                    config_cmd::handle_instances_list()?;
                }
                InstancesAction::Add { name } => {
                    config_cmd::handle_instances_add(&name)?;
                }
                InstancesAction::Remove { name } => {
                    config_cmd::handle_instances_remove(&name)?;
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
