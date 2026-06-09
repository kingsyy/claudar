use crate::config::NotificationsConfig;
use crate::notification_trait::NotificationSender;
use crate::pace;
use crate::state::LimitType;
use crate::time_format;
use chrono::{DateTime, Utc};
use notify_rust::Timeout;

/// Format a notification title with optional instance prefix.
/// When instance_name is "default", no prefix is added (backward compat).
fn format_title(instance_name: &str, title: &str) -> String {
    if instance_name == "default" {
        title.to_string()
    } else {
        format!("[{}] {}", instance_name, title)
    }
}

/// Send a threshold crossing notification
pub fn notify_threshold(
    sender: &dyn NotificationSender,
    config: &NotificationsConfig,
    instance_name: &str,
    limit_type: LimitType,
    percentage: f64,
    resets_in: &str,
    predicted_percentage: Option<f64>,
    reset_time: Option<DateTime<Utc>>,
    period_minutes: i64,
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

    let summary = format_title(
        instance_name,
        &format!(
            "{} Claude Usage Alert: {} Limit",
            emoji,
            limit_type.as_str()
        ),
    );

    let pace_line = if let Some(reset_time) = reset_time {
        let pace_info = pace::calculate_pace_info(percentage, reset_time, period_minutes);
        format!("\n{}", pace_info)
    } else {
        String::new()
    };

    let predicted_line = match predicted_percentage {
        Some(pred) if pred > 100.0 => format!("\nAt current pace: {:.0}% of limit", pred),
        Some(pred) => format!("\nAt current pace: {:.0}% of limit", pred),
        None => String::new(),
    };

    let body = format!(
        "You've used {:.0}% of your {} limit.\nResets in {}{}{}",
        percentage,
        limit_type.as_str(),
        resets_in,
        pace_line,
        predicted_line,
    );

    sender.send(&summary, &body, timeout, config.sound)?;

    Ok(())
}

/// Send a predicted overage notification
pub fn notify_predicted_overage(
    sender: &dyn NotificationSender,
    config: &NotificationsConfig,
    instance_name: &str,
    limit_type: LimitType,
    current_percentage: f64,
    predicted_percentage: f64,
    resets_in: &str,
    reset_time: Option<DateTime<Utc>>,
    period_minutes: i64,
) -> anyhow::Result<()> {
    if !config.notify_predicted_overage {
        return Ok(());
    }

    let timeout = if config.persistent {
        Timeout::Never
    } else {
        Timeout::Milliseconds(10000)
    };

    let summary = format_title(
        instance_name,
        &format!(
            "⚡ Claude Usage Warning: {} Limit",
            limit_type.as_str()
        ),
    );

    let pace_line = if let Some(reset_time) = reset_time {
        let pace_info = pace::calculate_pace_info(current_percentage, reset_time, period_minutes);
        format!("\n{}", pace_info)
    } else {
        String::new()
    };

    let body = format!(
        "At current pace, you'll use {:.0}% of your {} limit before reset.\n\
         Current: {:.0}% | Resets in: {}{}",
        predicted_percentage,
        limit_type.as_str(),
        current_percentage,
        resets_in,
        pace_line
    );

    sender.send(&summary, &body, timeout, config.sound)?;

    Ok(())
}

/// Send a reset notification
pub fn notify_reset(
    sender: &dyn NotificationSender,
    config: &NotificationsConfig,
    instance_name: &str,
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

    let summary = format_title(
        instance_name,
        &format!(
            "✓ Claude {} Limit Reset",
            limit_type.as_str()
        ),
    );
    let body = format!(
        "Your {} usage limit has been reset.\nYou now have fresh capacity available.",
        limit_type.as_str()
    );

    sender.send(&summary, &body, timeout, config.sound)?;

    Ok(())
}

/// Send an upcoming reset notification (X minutes before reset)
pub fn notify_upcoming_reset(
    sender: &dyn NotificationSender,
    config: &NotificationsConfig,
    instance_name: &str,
    limit_type: LimitType,
    current_percentage: f64,
    remaining_capacity: f64,
    resets_in: &str,
) -> anyhow::Result<()> {
    let timeout = if config.persistent {
        Timeout::Never
    } else {
        Timeout::Milliseconds(10000) // 10 seconds
    };

    let summary = format_title(
        instance_name,
        &format!(
            "⏰ Claude {} Limit Resetting Soon",
            limit_type.as_str()
        ),
    );
    let body = format!(
        "Your {} limit resets in {}.\n\
         Current usage: {:.0}% | Remaining: {:.0}%\n\
         Perfect time for token-intensive tasks!",
        limit_type.as_str(),
        resets_in,
        current_percentage,
        remaining_capacity
    );

    sender.send(&summary, &body, timeout, config.sound)?;

    Ok(())
}

/// Send an unused capacity warning (time + capacity threshold)
pub fn notify_unused_capacity(
    sender: &dyn NotificationSender,
    config: &NotificationsConfig,
    instance_name: &str,
    limit_type: LimitType,
    _current_percentage: f64,
    remaining_capacity: f64,
    resets_in: &str,
    other_limit_type: LimitType,
    other_percentage: f64,
    other_resets_at: chrono::DateTime<chrono::Utc>,
    timezone: &str,
) -> anyhow::Result<()> {
    let timeout = if config.persistent {
        Timeout::Never
    } else {
        Timeout::Milliseconds(10000) // 10 seconds
    };

    // Format the other limit's reset time as absolute date/time in configured timezone
    let other_reset_str = time_format::format_notification_time_24h(&other_resets_at, timezone)
        .unwrap_or_else(|_| {
            let other_local_time: chrono::DateTime<chrono::Local> = other_resets_at.into();
            other_local_time.format("%H:%M %d-%m-%Y").to_string()
        });
    let other_remaining = 100.0 - other_percentage;

    let summary = format_title(
        instance_name,
        &format!(
            "💡 Unused {} Capacity Warning",
            limit_type.as_str()
        ),
    );
    let body = format!(
        "Your {} limit resets in {} with {:.0}% unused capacity.\n\
         Consider using it for large tasks before it expires!\n\
         (Your {} limit has {:.0}% remaining, resets {})",
        limit_type.as_str(),
        resets_in,
        remaining_capacity,
        other_limit_type.as_str(),
        other_remaining,
        other_reset_str
    );

    sender.send(&summary, &body, timeout, config.sound)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification_trait::MockNotificationSender;

    fn default_config() -> NotificationsConfig {
        NotificationsConfig {
            sound: true,
            persistent: false,
            notify_threshold_crossings: true,
            notify_predicted_overage: true,
            notify_resets: true,
            minutes_before_five_hour_reset: None,
            minutes_before_seven_day_reset: None,
            capacity_warning_five_hour: None,
            capacity_warning_seven_day: None,
        }
    }

    // ========== Threshold Notification Tests ==========

    #[test]
    fn test_notify_threshold_sends_notification_when_enabled() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_threshold(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            75.0,
            "2h 30m",
            None,
            None,
            300,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].summary.contains("⚠️"));
        assert!(sent[0].summary.contains("5-hour"));
        assert!(sent[0].body.contains("75%"));
        assert!(sent[0].body.contains("2h 30m"));
        assert_eq!(sent[0].timeout_ms, Some(10000));
    }

    #[test]
    fn test_notify_threshold_skips_when_disabled() {
        let mock = MockNotificationSender::new();
        let mut config = default_config();
        config.notify_threshold_crossings = false;

        notify_threshold(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            75.0,
            "2h 30m",
            None,
            None,
            300,
        )
        .unwrap();

        assert_eq!(mock.count(), 0);
    }

    #[test]
    fn test_notify_threshold_emoji_90_percent() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_threshold(
            &mock,
            &config,
            "default",
            LimitType::SevenDay,
            95.0,
            "1d 5h",
            None,
            None,
            10080,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert!(sent[0].summary.contains("🔴"));
    }

    #[test]
    fn test_notify_threshold_emoji_70_percent() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_threshold(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            75.0,
            "1h",
            None,
            None,
            300,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert!(sent[0].summary.contains("⚠️"));
    }

    #[test]
    fn test_notify_threshold_emoji_below_70_percent() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_threshold(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            50.0,
            "2h",
            None,
            None,
            300,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert!(sent[0].summary.contains("ℹ️"));
    }

    #[test]
    fn test_notify_threshold_persistent_timeout() {
        let mock = MockNotificationSender::new();
        let mut config = default_config();
        config.persistent = true;

        notify_threshold(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            90.0,
            "30m",
            None,
            None,
            300,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert_eq!(sent[0].timeout_ms, None); // Timeout::Never
    }

    // ========== Predicted Overage Tests ==========

    #[test]
    fn test_notify_predicted_overage_sends_when_enabled() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_predicted_overage(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            60.0,
            120.0,
            "1h 30m",
            None,
            300,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].summary.contains("⚡"));
        assert!(sent[0].summary.contains("5-hour"));
        assert!(sent[0].body.contains("120%"));
        assert!(sent[0].body.contains("60%"));
        assert!(sent[0].body.contains("1h 30m"));
    }

    #[test]
    fn test_notify_predicted_overage_skips_when_disabled() {
        let mock = MockNotificationSender::new();
        let mut config = default_config();
        config.notify_predicted_overage = false;

        notify_predicted_overage(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            60.0,
            120.0,
            "1h 30m",
            None,
            300,
        )
        .unwrap();

        assert_eq!(mock.count(), 0);
    }

    // ========== Reset Notification Tests ==========

    #[test]
    fn test_notify_reset_sends_when_enabled() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_reset(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].summary.contains("✓"));
        assert!(sent[0].summary.contains("5-hour"));
        assert!(sent[0].body.contains("reset"));
        assert_eq!(sent[0].timeout_ms, Some(8000));
    }

    #[test]
    fn test_notify_reset_skips_when_disabled() {
        let mock = MockNotificationSender::new();
        let mut config = default_config();
        config.notify_resets = false;

        notify_reset(
            &mock,
            &config,
            "default",
            LimitType::SevenDay,
        )
        .unwrap();

        assert_eq!(mock.count(), 0);
    }

    #[test]
    fn test_notify_reset_seven_day_limit() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_reset(
            &mock,
            &config,
            "default",
            LimitType::SevenDay,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert!(sent[0].summary.contains("7-day"));
    }

    // ========== Upcoming Reset Notification Tests ==========

    #[test]
    fn test_notify_upcoming_reset_sends_notification() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_upcoming_reset(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            65.0,
            35.0,
            "25m",
        )
        .unwrap();

        let sent = mock.get_sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].summary.contains("⏰"));
        assert!(sent[0].summary.contains("5-hour"));
        assert!(sent[0].body.contains("65%"));
        assert!(sent[0].body.contains("35%"));
        assert!(sent[0].body.contains("25m"));
        assert!(sent[0].body.contains("token-intensive"));
    }

    #[test]
    fn test_notify_upcoming_reset_seven_day() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_upcoming_reset(
            &mock,
            &config,
            "default",
            LimitType::SevenDay,
            80.0,
            20.0,
            "1h",
        )
        .unwrap();

        let sent = mock.get_sent();
        assert!(sent[0].summary.contains("7-day"));
    }

    // ========== Instance Name Prefix Tests ==========

    #[test]
    fn test_notify_threshold_with_instance_prefix() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_threshold(
            &mock,
            &config,
            "work",
            LimitType::FiveHour,
            90.0,
            "1h",
            None,
            None,
            300,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert!(sent[0].summary.contains("[work]"));
    }

    #[test]
    fn test_notify_threshold_default_no_prefix() {
        let mock = MockNotificationSender::new();
        let config = default_config();

        notify_threshold(
            &mock,
            &config,
            "default",
            LimitType::FiveHour,
            90.0,
            "1h",
            None,
            None,
            300,
        )
        .unwrap();

        let sent = mock.get_sent();
        assert!(!sent[0].summary.contains("[default]"));
    }

    // ========== Duration Formatting Tests ==========

    #[test]
    fn test_format_duration_weeks() {
        let duration = chrono::Duration::weeks(2) + chrono::Duration::days(3);
        assert_eq!(format_duration(duration), "2w 3d");
    }

    #[test]
    fn test_format_duration_days() {
        let duration = chrono::Duration::days(3) + chrono::Duration::hours(5);
        assert_eq!(format_duration(duration), "3d 5h");
    }

    #[test]
    fn test_format_duration_hours() {
        let duration = chrono::Duration::hours(4) + chrono::Duration::minutes(30);
        assert_eq!(format_duration(duration), "4h 30m");
    }

    #[test]
    fn test_format_duration_minutes() {
        let duration = chrono::Duration::minutes(45);
        assert_eq!(format_duration(duration), "45m");
    }

    #[test]
    fn test_format_duration_less_than_minute() {
        let duration = chrono::Duration::seconds(30);
        assert_eq!(format_duration(duration), "< 1m");
    }
}
