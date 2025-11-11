mod browser_auth;
mod cli;
mod config;
mod setup;
mod status;
mod storage;

use clap::Parser;
use cli::{Cli, Commands};
use tracing_subscriber;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Route to appropriate command handler
    match cli.command {
        Commands::Setup => {
            setup::run_setup()?;
        }
        Commands::Run => {
            println!("Run command not yet implemented.");
            println!("This will start the monitoring daemon.");
        }
        Commands::Status => {
            status::run_status()?;
        }
    }

    Ok(())
}
