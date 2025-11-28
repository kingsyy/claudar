use crate::config::Config;
use crate::service;
use crate::storage::SessionData;
use colored::Colorize;

pub fn run_status(_verbose: bool) -> anyhow::Result<()> {
    println!("\n{}", "Claude Notify Status".bold());
    println!("{}", "━".repeat(50));
    println!();

    // Version Info Section
    display_version_info();
    println!();

    // Service Status Section
    display_service_status()?;
    println!();

    // Configuration Section
    display_configuration()?;
    println!();

    // Session Info Section
    display_session_info()?;
    println!();

    Ok(())
}

fn display_version_info() {
    let version = env!("CARGO_PKG_VERSION");
    let commit = env!("GIT_HASH");

    println!("{}", "Version Info".bold().underline());
    println!();
    println!("  Version:  {}", version.cyan());
    println!("  Commit:   {}", commit.bright_black());
}

fn display_service_status() -> anyhow::Result<()> {
    println!("{}", "Service Status".bold().underline());
    println!();

    let installed = service::is_service_installed();
    let running = if installed {
        service::is_service_running().unwrap_or(false)
    } else {
        false
    };

    // Display status with icons and colors
    if running {
        println!("  Status:     {} {}", "●".green(), "Running".green().bold());
    } else if installed {
        println!("  Status:     {} {}", "○".yellow(), "Stopped".yellow());
        println!("  {}  Run 'claude-notify start' to start the service", "💡".bright_blue());
    } else {
        println!("  Status:     {} {}", "○".bright_black(), "Not installed".bright_black());
        println!("  {}  Run 'claude-notify setup-service' to install", "💡".bright_blue());
    }

    if installed {
        if let Ok(path) = service::get_service_path() {
            println!("  Location:   {}", path.display().to_string().bright_black());
        }
    }

    Ok(())
}

fn display_configuration() -> anyhow::Result<()> {
    println!("{}", "Configuration".bold().underline());
    println!();

    let config = Config::load()?;

    // Poll Interval
    let interval_seconds = config.general.poll_interval_seconds;
    let interval_display = if interval_seconds >= 60 {
        format!("{} minutes", interval_seconds / 60)
    } else {
        format!("{} seconds", interval_seconds)
    };
    println!("  Poll Interval:          {}", interval_display.cyan());

    // Thresholds
    let five_hour_thresholds = config.thresholds.five_hour
        .iter()
        .map(|t| format!("{}%", t))
        .collect::<Vec<_>>()
        .join(", ");
    println!("  5-Hour Thresholds:      {}", five_hour_thresholds.cyan());

    let seven_day_thresholds = config.thresholds.seven_day
        .iter()
        .map(|t| format!("{}%", t))
        .collect::<Vec<_>>()
        .join(", ");
    println!("  7-Day Thresholds:       {}", seven_day_thresholds.cyan());

    // Notifications
    println!();
    println!("  {}:", "Notifications".bold());
    println!("    Sound:                {}", format_bool(config.notifications.sound));
    println!("    Persistent:           {}", format_bool(config.notifications.persistent));
    println!("    Threshold Crossings:  {}", format_bool(config.notifications.notify_threshold_crossings));
    println!("    Predicted Overage:    {}", format_bool(config.notifications.notify_predicted_overage));
    println!("    Resets:               {}", format_bool(config.notifications.notify_resets));

    println!();
    println!("  {}  Use 'claude-notify config' to modify settings", "💡".bright_blue());

    Ok(())
}

fn display_session_info() -> anyhow::Result<()> {
    println!("{}", "Session Info".bold().underline());
    println!();

    let config = Config::load()?;

    match SessionData::load(&config.auth.session_file) {
        Ok(session) => {
            println!("  Status:          {} {}", "✓".green(), "Authenticated".green());
            println!("  Organization ID: {}", session.org_id.cyan());

            if let Some(ref org) = session.last_active_org {
                println!("  Last Active Org: {}", org.bright_black());
            }
        }
        Err(_) => {
            println!("  Status:          {} {}", "✗".red(), "Not authenticated".red());
            println!("  {}  Run 'claude-notify setup' to authenticate", "💡".bright_blue());
        }
    }

    Ok(())
}

fn format_bool(value: bool) -> colored::ColoredString {
    if value {
        "Enabled".green()
    } else {
        "Disabled".bright_black()
    }
}
