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
    println!("[instances]");
    if config.instances.is_empty() {
        println!("  (none configured - using single default instance)");
    } else {
        for instance in &config.instances {
            println!("  - {}", instance.name);
        }
    }
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

pub fn handle_instances_list() -> Result<()> {
    let config = Config::load()?;

    if config.instances.is_empty() {
        println!("No instances configured (using single default instance).");
        println!("\nTo add instances:");
        println!("  claude-notify instances add personal");
        println!("  claude-notify instances add work");
        return Ok(());
    }

    println!("Configured instances:");
    for instance in &config.instances {
        println!("  - {}", instance.name);
    }

    Ok(())
}

pub fn handle_instances_add(name: &str) -> Result<()> {
    let mut config = Config::load()?;

    // Validate name
    if name.is_empty() {
        return Err(anyhow!("Instance name cannot be empty"));
    }
    if name == "default" {
        return Err(anyhow!("'default' is a reserved instance name"));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(anyhow!("Instance name can only contain alphanumeric characters, hyphens, and underscores"));
    }

    // Check for duplicate
    if config.instances.iter().any(|i| i.name == name) {
        return Err(anyhow!("Instance '{}' already exists", name));
    }

    config.instances.push(crate::config::InstanceConfig { name: name.to_string() });
    config.save()?;

    println!("✓ Added instance '{}'", name);
    println!("\nNext step: Run setup for this instance:");
    println!("  claude-notify setup --instance {}", name);

    Ok(())
}

pub fn handle_instances_remove(name: &str) -> Result<()> {
    let mut config = Config::load()?;

    let original_len = config.instances.len();
    config.instances.retain(|i| i.name != name);

    if config.instances.len() == original_len {
        return Err(anyhow!("Instance '{}' not found", name));
    }

    config.save()?;

    // Clean up session and state files
    if let Ok(session_path) = config.session_path_for(name) {
        if session_path.exists() {
            let _ = std::fs::remove_file(&session_path);
        }
    }
    if let Ok(state_path) = config.state_path_for(name) {
        if state_path.exists() {
            let _ = std::fs::remove_file(&state_path);
        }
    }

    println!("✓ Removed instance '{}' and its associated files", name);

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_threshold_list ---

    #[test]
    fn parse_threshold_list_valid_multiple() {
        let result = parse_threshold_list("50,70,90").unwrap();
        assert_eq!(result, vec![50, 70, 90]);
    }

    #[test]
    fn parse_threshold_list_single_value() {
        let result = parse_threshold_list("80").unwrap();
        assert_eq!(result, vec![80]);
    }

    #[test]
    fn parse_threshold_list_with_spaces() {
        let result = parse_threshold_list("50, 70, 90").unwrap();
        assert_eq!(result, vec![50, 70, 90]);
    }

    #[test]
    fn parse_threshold_list_invalid_string() {
        assert!(parse_threshold_list("abc").is_err());
    }

    #[test]
    fn parse_threshold_list_boundary_values() {
        let result = parse_threshold_list("0,100").unwrap();
        assert_eq!(result, vec![0, 100]);
    }

    // --- parse_bool ---

    #[test]
    fn parse_bool_truthy_values() {
        for val in &["true", "yes", "1", "on", "True", "YES", "On"] {
            assert_eq!(parse_bool(val).unwrap(), true, "failed for {}", val);
        }
    }

    #[test]
    fn parse_bool_falsy_values() {
        for val in &["false", "no", "0", "off", "False", "NO", "Off"] {
            assert_eq!(parse_bool(val).unwrap(), false, "failed for {}", val);
        }
    }

    #[test]
    fn parse_bool_invalid() {
        assert!(parse_bool("maybe").is_err());
        assert!(parse_bool("").is_err());
    }

    // --- parse_optional_minutes ---

    #[test]
    fn parse_optional_minutes_numeric() {
        assert_eq!(parse_optional_minutes("30").unwrap(), Some(30));
        assert_eq!(parse_optional_minutes("1").unwrap(), Some(1));
    }

    #[test]
    fn parse_optional_minutes_disabled_variants() {
        for val in &["disabled", "none", "off", "0", "Disabled", "NONE", "OFF"] {
            assert_eq!(parse_optional_minutes(val).unwrap(), None, "failed for {}", val);
        }
    }

    #[test]
    fn parse_optional_minutes_invalid() {
        assert!(parse_optional_minutes("abc").is_err());
        assert!(parse_optional_minutes("-5").is_err());
    }

    // --- parse_capacity_warning ---

    #[test]
    fn parse_capacity_warning_valid() {
        assert_eq!(parse_capacity_warning("30,20").unwrap(), Some((30, 20)));
    }

    #[test]
    fn parse_capacity_warning_with_spaces() {
        assert_eq!(parse_capacity_warning("30, 20").unwrap(), Some((30, 20)));
    }

    #[test]
    fn parse_capacity_warning_disabled() {
        for val in &["disabled", "none", "off", "Disabled"] {
            assert_eq!(parse_capacity_warning(val).unwrap(), None, "failed for {}", val);
        }
    }

    #[test]
    fn parse_capacity_warning_wrong_format() {
        assert!(parse_capacity_warning("30").is_err());
        assert!(parse_capacity_warning("30,20,10").is_err());
    }

    #[test]
    fn parse_capacity_warning_zero_minutes() {
        assert!(parse_capacity_warning("0,20").is_err());
    }

    #[test]
    fn parse_capacity_warning_percentage_over_100() {
        assert!(parse_capacity_warning("30,101").is_err());
    }

    // --- format_vec ---

    #[test]
    fn format_vec_basic() {
        assert_eq!(format_vec(&[50, 70, 90]), "50,70,90");
    }

    #[test]
    fn format_vec_single() {
        assert_eq!(format_vec(&[42]), "42");
    }

    #[test]
    fn format_vec_empty() {
        assert_eq!(format_vec(&[]), "");
    }

    // --- validate_notification_window ---

    #[test]
    fn validate_notification_window_ok() {
        // Should not error even when window < poll interval (it just warns)
        assert!(validate_notification_window(5, 900).is_ok());
        assert!(validate_notification_window(30, 900).is_ok());
    }
}
