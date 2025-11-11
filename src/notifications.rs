use crate::config::NotificationsConfig;
use crate::state::LimitType;
use notify_rust::{Notification, Timeout};

/// Send a threshold crossing notification
pub fn notify_threshold(
    config: &NotificationsConfig,
    limit_type: LimitType,
    percentage: f64,
    resets_in: &str,
) -> anyhow::Result<()> {
    if !config.notify_threshold_crossings {
        return Ok(());
    }

    let emoji = match percentage as u8 {
        90..=100 => "🔴",
        70..=89 => "⚠️",
        _ => "ℹ️",
    };

    let timeout = if config.persistent {
        Timeout::Never
    } else {
        Timeout::Milliseconds(10000) // 10 seconds
    };

    Notification::new()
        .summary(&format!(
            "{} Claude Usage Alert: {} Limit",
            emoji,
            limit_type.as_str()
        ))
        .body(&format!(
            "You've used {:.0}% of your {} limit.\nResets in {}",
            percentage,
            limit_type.as_str(),
            resets_in
        ))
        .timeout(timeout)
        .show()?;

    Ok(())
}

/// Send a predicted overage notification
pub fn notify_predicted_overage(
    config: &NotificationsConfig,
    limit_type: LimitType,
    current_percentage: f64,
    predicted_percentage: f64,
    resets_in: &str,
) -> anyhow::Result<()> {
    if !config.notify_predicted_overage {
        return Ok(());
    }

    let timeout = if config.persistent {
        Timeout::Never
    } else {
        Timeout::Milliseconds(10000)
    };

    Notification::new()
        .summary(&format!(
            "⚡ Claude Usage Warning: {} Limit",
            limit_type.as_str()
        ))
        .body(&format!(
            "At current pace, you'll use {:.0}% of your {} limit before reset.\n\
             Current: {:.0}% | Resets in: {}",
            predicted_percentage,
            limit_type.as_str(),
            current_percentage,
            resets_in
        ))
        .timeout(timeout)
        .show()?;

    Ok(())
}

/// Send a reset notification
pub fn notify_reset(
    config: &NotificationsConfig,
    limit_type: LimitType,
) -> anyhow::Result<()> {
    if !config.notify_resets {
        return Ok(());
    }

    let timeout = if config.persistent {
        Timeout::Never
    } else {
        Timeout::Milliseconds(8000) // 8 seconds
    };

    Notification::new()
        .summary(&format!(
            "✓ Claude {} Limit Reset",
            limit_type.as_str()
        ))
        .body(&format!(
            "Your {} usage limit has been reset.\nYou now have fresh capacity available.",
            limit_type.as_str()
        ))
        .timeout(timeout)
        .show()?;

    Ok(())
}

/// Format duration for human-readable display
pub fn format_duration(duration: chrono::Duration) -> String {
    if duration.num_weeks() > 0 {
        let weeks = duration.num_weeks();
        let days = duration.num_days() % 7;
        format!("{}w {}d", weeks, days)
    } else if duration.num_days() > 0 {
        let days = duration.num_days();
        let hours = duration.num_hours() % 24;
        format!("{}d {}h", days, hours)
    } else if duration.num_hours() > 0 {
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        format!("{}h {}m", hours, minutes)
    } else if duration.num_minutes() > 0 {
        let minutes = duration.num_minutes();
        format!("{}m", minutes)
    } else {
        "< 1m".to_string()
    }
}
