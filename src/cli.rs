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

    /// Check current Claude API usage statistics
    Usage,

    /// Check service status, configuration, and session info
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
        Displays all configuration settings:\n\n\
        [general]\n  \
        poll_interval_seconds               - How often to check API usage (default: 900s / 15min)\n\n\
        [thresholds]\n  \
        five_hour                           - Usage % thresholds for 5-hour limit alerts (default: 50,70,90)\n  \
        seven_day                           - Usage % thresholds for 7-day limit alerts (default: 50,70,90)\n\n\
        [notifications]\n  \
        sound                               - Enable notification sounds (default: true, not yet implemented)\n  \
        persistent                          - Keep notifications on screen until dismissed (default: false)\n  \
        notify_threshold_crossings          - Alert when crossing usage thresholds (default: true)\n  \
        notify_predicted_overage            - Warn if predicted to exceed 5-hour limit (default: true)\n  \
        notify_resets                       - Notify when usage limits reset (default: true)\n  \
        minutes_before_five_hour_reset      - Minutes before 5-hour reset to alert (default: disabled)\n  \
        minutes_before_seven_day_reset      - Minutes before 7-day reset to alert (default: disabled)")]
    List,

    /// Get a specific configuration value
    #[command(long_about = "Get a specific configuration value\n\n\
        VALID KEYS:\n  \
        general.poll_interval_seconds                    - How often to check API usage (default: 900)\n  \
        thresholds.five_hour                             - Usage % for 5-hour limit alerts (default: 50,70,90)\n  \
        thresholds.seven_day                             - Usage % for 7-day limit alerts (default: 50,70,90)\n  \
        notifications.sound                              - Enable notification sounds (default: true)\n  \
        notifications.persistent                         - Keep notifications on screen (default: false)\n  \
        notifications.notify_threshold_crossings         - Alert on threshold crossings (default: true)\n  \
        notifications.notify_predicted_overage           - Warn on predicted overage (default: true)\n  \
        notifications.notify_resets                      - Notify on limit resets (default: true)\n  \
        notifications.minutes_before_five_hour_reset     - Alert X min before 5-hour reset (default: disabled)\n  \
        notifications.minutes_before_seven_day_reset     - Alert X min before 7-day reset (default: disabled)\n\n\
        EXAMPLE:\n  \
        claude-notify config get general.poll_interval_seconds")]
    Get {
        /// Configuration key (e.g., "general.poll_interval_seconds")
        key: String,
    },

    /// Set a configuration value
    #[command(long_about = "Set a configuration value\n\n\
        VALID KEYS:\n  \
        general.poll_interval_seconds                    - How often to check API usage (number, min: 60)\n  \
        thresholds.five_hour                             - Usage % for 5-hour alerts (comma-separated: 50,70,90)\n  \
        thresholds.seven_day                             - Usage % for 7-day alerts (comma-separated: 50,70,90)\n  \
        notifications.sound                              - Enable notification sounds (true/false)\n  \
        notifications.persistent                         - Keep notifications on screen (true/false)\n  \
        notifications.notify_threshold_crossings         - Alert on threshold crossings (true/false)\n  \
        notifications.notify_predicted_overage           - Warn on predicted overage (true/false)\n  \
        notifications.notify_resets                      - Notify on limit resets (true/false)\n  \
        notifications.minutes_before_five_hour_reset     - Alert X min before 5-hour reset (number or 'disabled')\n  \
        notifications.minutes_before_seven_day_reset     - Alert X min before 7-day reset (number or 'disabled')\n\n\
        VALUE FORMATS:\n  \
        Numbers:     600, 900, 3600\n  \
        Thresholds:  50,70,90 (comma-separated, no spaces)\n  \
        Booleans:    true/false, yes/no, 1/0, on/off\n\n\
        EXAMPLES:\n  \
        claude-notify config set general.poll_interval_seconds 600\n  \
        claude-notify config set thresholds.five_hour 50,75,90,95\n  \
        claude-notify config set notifications.sound false\n  \
        claude-notify config set notifications.minutes_before_five_hour_reset 30\n\n\
        NOTE: Restart the service after changing config:\n  \
        claude-notify stop && claude-notify start")]
    Set {
        /// Configuration key (e.g., "general.poll_interval_seconds")
        key: String,
        /// New value (format depends on key type)
        value: String,
    },
}
