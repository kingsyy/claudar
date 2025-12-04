use crate::config::Config;
use crate::time_format;
use anyhow::{anyhow, Context, Result};

pub fn handle_config_list() -> Result<()> {
    let config = Config::load()?;

    println!("Current Configuration:");
    println!();
    println!("[general]");
    println!("  poll_interval_seconds = {}", config.general.poll_interval_seconds);
    println!("  timezone = {}", config.general.timezone);
    println!();
    println!("[thresholds]");
    println!("  five_hour = {:?}", config.thresholds.five_hour);
    println!("  seven_day = {:?}", config.thresholds.seven_day);
    println!();
    println!("[notifications]");
    println!("  sound = {}", config.notifications.sound);
    println!("  persistent = {}", config.notifications.persistent);
    println!("  notify_threshold_crossings = {}", config.notifications.notify_threshold_crossings);
    println!("  notify_predicted_overage = {}", config.notifications.notify_predicted_overage);
    println!("  notify_resets = {}", config.notifications.notify_resets);
    println!("  minutes_before_five_hour_reset = {}",
        config.notifications.minutes_before_five_hour_reset
            .map(|m| m.to_string())
            .unwrap_or_else(|| "disabled".to_string()));
    println!("  minutes_before_seven_day_reset = {}",
        config.notifications.minutes_before_seven_day_reset
            .map(|m| m.to_string())
            .unwrap_or_else(|| "disabled".to_string()));
    println!("  capacity_warning_five_hour = {}",
        config.notifications.capacity_warning_five_hour
            .map(|(min, pct)| format!("{},{}", min, pct))
            .unwrap_or_else(|| "disabled".to_string()));
    println!("  capacity_warning_seven_day = {}",
        config.notifications.capacity_warning_seven_day
            .map(|(min, pct)| format!("{},{}", min, pct))
            .unwrap_or_else(|| "disabled".to_string()));
    println!();
    println!("Config file location: {}", Config::config_path()?.display());

    Ok(())
}

pub fn handle_config_get(key: &str) -> Result<()> {
    let config = Config::load()?;

    let value = match key {
        "general.poll_interval_seconds" => config.general.poll_interval_seconds.to_string(),
        "general.timezone" => config.general.timezone.clone(),
        "thresholds.five_hour" => format_vec(&config.thresholds.five_hour),
        "thresholds.seven_day" => format_vec(&config.thresholds.seven_day),
        "notifications.sound" => config.notifications.sound.to_string(),
        "notifications.persistent" => config.notifications.persistent.to_string(),
        "notifications.notify_threshold_crossings" => config.notifications.notify_threshold_crossings.to_string(),
        "notifications.notify_predicted_overage" => config.notifications.notify_predicted_overage.to_string(),
        "notifications.notify_resets" => config.notifications.notify_resets.to_string(),
        "notifications.minutes_before_five_hour_reset" => {
            config.notifications.minutes_before_five_hour_reset
                .map(|m| m.to_string())
                .unwrap_or_else(|| "disabled".to_string())
        }
        "notifications.minutes_before_seven_day_reset" => {
            config.notifications.minutes_before_seven_day_reset
                .map(|m| m.to_string())
                .unwrap_or_else(|| "disabled".to_string())
        }
        "notifications.capacity_warning_five_hour" => {
            config.notifications.capacity_warning_five_hour
                .map(|(min, pct)| format!("{},{}", min, pct))
                .unwrap_or_else(|| "disabled".to_string())
        }
        "notifications.capacity_warning_seven_day" => {
            config.notifications.capacity_warning_seven_day
                .map(|(min, pct)| format!("{},{}", min, pct))
                .unwrap_or_else(|| "disabled".to_string())
        }
        _ => return Err(anyhow!("Unknown config key: {}\n\nAvailable keys:\n  \
            general.poll_interval_seconds\n  \
            general.timezone\n  \
            thresholds.five_hour\n  \
            thresholds.seven_day\n  \
            notifications.sound\n  \
            notifications.persistent\n  \
            notifications.notify_threshold_crossings\n  \
            notifications.notify_predicted_overage\n  \
            notifications.notify_resets\n  \
            notifications.minutes_before_five_hour_reset\n  \
            notifications.minutes_before_seven_day_reset\n  \
            notifications.capacity_warning_five_hour\n  \
            notifications.capacity_warning_seven_day", key)),
    };

    println!("{} = {}", key, value);
    Ok(())
}

pub fn handle_config_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;

    match key {
        "general.poll_interval_seconds" => {
            let val: u64 = value.parse()
                .context("Value must be a positive integer (seconds)")?;
            if val < 60 {
                return Err(anyhow!("Poll interval must be at least 60 seconds"));
            }
            config.general.poll_interval_seconds = val;
        }
        "general.timezone" => {
            // Validate timezone before setting
            time_format::validate_timezone(value)?;
            config.general.timezone = value.to_string();
        }
        "thresholds.five_hour" => {
            config.thresholds.five_hour = parse_threshold_list(value)?;
        }
        "thresholds.seven_day" => {
            config.thresholds.seven_day = parse_threshold_list(value)?;
        }
        "notifications.sound" => {
            config.notifications.sound = parse_bool(value)?;
        }
        "notifications.persistent" => {
            config.notifications.persistent = parse_bool(value)?;
        }
        "notifications.notify_threshold_crossings" => {
            config.notifications.notify_threshold_crossings = parse_bool(value)?;
        }
        "notifications.notify_predicted_overage" => {
            config.notifications.notify_predicted_overage = parse_bool(value)?;
        }
        "notifications.notify_resets" => {
            config.notifications.notify_resets = parse_bool(value)?;
        }
        "notifications.minutes_before_five_hour_reset" => {
            let val = parse_optional_minutes(value)?;
            if let Some(minutes) = val {
                validate_notification_window(minutes, config.general.poll_interval_seconds)?;
            }
            config.notifications.minutes_before_five_hour_reset = val;
        }
        "notifications.minutes_before_seven_day_reset" => {
            let val = parse_optional_minutes(value)?;
            if let Some(minutes) = val {
                validate_notification_window(minutes, config.general.poll_interval_seconds)?;
            }
            config.notifications.minutes_before_seven_day_reset = val;
        }
        "notifications.capacity_warning_five_hour" => {
            let val = parse_capacity_warning(value)?;
            if let Some((minutes, _)) = val {
                validate_notification_window(minutes, config.general.poll_interval_seconds)?;
            }
            config.notifications.capacity_warning_five_hour = val;
        }
        "notifications.capacity_warning_seven_day" => {
            let val = parse_capacity_warning(value)?;
            if let Some((minutes, _)) = val {
                validate_notification_window(minutes, config.general.poll_interval_seconds)?;
            }
            config.notifications.capacity_warning_seven_day = val;
        }
        _ => return Err(anyhow!("Unknown config key: {}\n\nAvailable keys:\n  \
            general.poll_interval_seconds\n  \
            general.timezone\n  \
            thresholds.five_hour\n  \
            thresholds.seven_day\n  \
            notifications.sound\n  \
            notifications.persistent\n  \
            notifications.notify_threshold_crossings\n  \
            notifications.notify_predicted_overage\n  \
            notifications.notify_resets\n  \
            notifications.minutes_before_five_hour_reset\n  \
            notifications.minutes_before_seven_day_reset\n  \
            notifications.capacity_warning_five_hour\n  \
            notifications.capacity_warning_seven_day", key)),
    }

    config.save()?;
    println!("✓ Updated {} = {}", key, value);
    println!("\nNote: If the monitor service is running, restart it for changes to take effect:");
    println!("  claude-notify stop && claude-notify start");

    Ok(())
}

fn format_vec(vec: &[u8]) -> String {
    vec.iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_threshold_list(value: &str) -> Result<Vec<u8>> {
    let thresholds: Result<Vec<u8>> = value
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<u8>()
                .context("Each threshold must be a number between 0-100")
        })
        .collect();

    let thresholds = thresholds?;

    // Validate thresholds
    for &t in &thresholds {
        if t > 100 {
            return Err(anyhow!("Threshold values must be between 0-100"));
        }
    }

    Ok(thresholds)
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(anyhow!("Value must be a boolean (true/false, yes/no, 1/0, on/off)")),
    }
}

fn parse_optional_minutes(value: &str) -> Result<Option<u64>> {
    match value.to_lowercase().as_str() {
        "disabled" | "none" | "off" | "0" => Ok(None),
        _ => {
            let minutes: u64 = value.parse()
                .context("Value must be a positive number (minutes) or 'disabled'")?;
            if minutes == 0 {
                Ok(None)
            } else {
                Ok(Some(minutes))
            }
        }
    }
}

fn parse_capacity_warning(value: &str) -> Result<Option<(u64, u8)>> {
    match value.to_lowercase().as_str() {
        "disabled" | "none" | "off" => Ok(None),
        _ => {
            let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(anyhow!(
                    "Value must be in format 'minutes,percentage' (e.g., '30,20') or 'disabled'"
                ));
            }

            let minutes: u64 = parts[0]
                .parse()
                .context("Minutes must be a positive number")?;
            let percentage: u8 = parts[1]
                .parse()
                .context("Percentage must be a number between 0-100")?;

            if minutes == 0 {
                return Err(anyhow!("Minutes must be greater than 0"));
            }
            if percentage > 100 {
                return Err(anyhow!("Percentage must be between 0-100"));
            }

            Ok(Some((minutes, percentage)))
        }
    }
}

fn validate_notification_window(notification_minutes: u64, poll_interval_seconds: u64) -> Result<()> {
    let poll_interval_minutes = poll_interval_seconds / 60;

    if notification_minutes < poll_interval_minutes {
        println!("\n⚠️  Warning: Notification window ({} minutes) is smaller than poll interval ({} minutes).",
            notification_minutes, poll_interval_minutes);
        println!("    You may miss notifications if the reset occurs between polling cycles.");
        println!("    Consider either:");
        println!("      - Increasing the notification window to at least {} minutes", poll_interval_minutes);
        println!("      - Decreasing the poll interval (currently {} seconds)", poll_interval_seconds);
        println!();
    }

    Ok(())
}
