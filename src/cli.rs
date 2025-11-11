use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "claude-notify")]
#[command(about = "Monitor Claude Code usage and get notifications", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Interactive setup wizard to configure authentication
    Setup,

    /// Run the monitor daemon in foreground
    Run,

    /// Check current usage status
    Status,
}
