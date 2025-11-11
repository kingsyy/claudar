use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "claude-notify")]
#[command(about = "Monitor Claude Code usage and get notifications", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output (shows all logs and debug information)
    #[arg(long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Interactive setup wizard to configure authentication
    Setup,

    /// Run the monitor daemon in foreground mode
    Run,

    /// Check current usage status
    Status,

    /// Manage configuration settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Install the monitor as a system service (launchd/systemd)
    #[command(name = "setup-service")]
    SetupService,

    /// Start the background service
    Start,

    /// Stop the background service
    Stop,

    /// Uninstall the system service
    #[command(name = "uninstall-service")]
    UninstallService,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// List all configuration values
    List,

    /// Get a specific configuration value
    Get {
        /// Configuration key (e.g., "general.poll_interval_seconds")
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., "general.poll_interval_seconds")
        key: String,
        /// New value
        value: String,
    },
}
