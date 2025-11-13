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
    ///
    /// View and modify configuration values for polling intervals, thresholds,
    /// and notification preferences. Changes require service restart if running.
    #[command(long_about = "Manage configuration settings\n\n\
        View and modify configuration values including:\n\
        - Poll interval (how often to check usage)\n\
        - Thresholds for 5-hour and 7-day limits\n\
        - Notification preferences (sound, persistence, types)\n\n\
        EXAMPLES:\n  \
        claude-notify config list\n  \
        claude-notify config get general.poll_interval_seconds\n  \
        claude-notify config set thresholds.five_hour 50,75,90,95\n  \
        claude-notify config set notifications.sound false")]
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
    /// List all configuration values with their current settings
    #[command(long_about = "List all configuration values\n\n\
        Displays all configuration settings including:\n\
        - general.poll_interval_seconds\n\
        - thresholds.five_hour\n\
        - thresholds.seven_day\n\
        - notifications.sound\n\
        - notifications.persistent\n\
        - notifications.notify_threshold_crossings\n\
        - notifications.notify_predicted_overage\n\
        - notifications.notify_resets")]
    List,

    /// Get a specific configuration value
    #[command(long_about = "Get a specific configuration value\n\n\
        VALID KEYS:\n  \
        general.poll_interval_seconds    - Poll interval in seconds (min: 60)\n  \
        thresholds.five_hour             - 5-hour limit thresholds (e.g., 50,70,90)\n  \
        thresholds.seven_day             - 7-day limit thresholds (e.g., 50,70,90)\n  \
        notifications.sound              - Enable notification sounds (true/false)\n  \
        notifications.persistent         - Keep notifications on screen (true/false)\n  \
        notifications.notify_threshold_crossings - Alert on threshold crossings (true/false)\n  \
        notifications.notify_predicted_overage   - Warn on predicted overage (true/false)\n  \
        notifications.notify_resets              - Notify on limit resets (true/false)\n\n\
        EXAMPLE:\n  \
        claude-notify config get general.poll_interval_seconds")]
    Get {
        /// Configuration key (e.g., "general.poll_interval_seconds")
        key: String,
    },

    /// Set a configuration value
    #[command(long_about = "Set a configuration value\n\n\
        VALUE FORMATS:\n  \
        Numbers:     600, 900, 3600\n  \
        Thresholds:  50,70,90 (comma-separated, no spaces)\n  \
        Booleans:    true/false, yes/no, 1/0, on/off\n\n\
        EXAMPLES:\n  \
        claude-notify config set general.poll_interval_seconds 600\n  \
        claude-notify config set thresholds.five_hour 50,75,90,95\n  \
        claude-notify config set notifications.sound false\n\n\
        NOTE: Restart the service after changing config:\n  \
        claude-notify stop && claude-notify start")]
    Set {
        /// Configuration key (e.g., "general.poll_interval_seconds")
        key: String,
        /// New value (format depends on key type)
        value: String,
    },
}
