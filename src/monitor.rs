use crate::browser_auth::BrowserAuthenticator;
use crate::config::Config;
use crate::notification_trait::{NotificationSender, RealNotificationSender};
use crate::notifications::{
    format_duration, notify_predicted_overage, notify_reset, notify_threshold,
    notify_upcoming_reset, notify_unused_capacity,
};
use crate::state::{LimitType, MonitorState};
use crate::storage::SessionData;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct UsageResponse {
    five_hour: UsageLimit,
    seven_day: UsageLimit,
}

#[derive(Debug, Deserialize)]
struct UsageLimit {
    utilization: f64,
    #[serde(default)]
    resets_at: Option<String>,
}

pub fn run_monitor(foreground: bool) -> anyhow::Result<()> {
    if foreground {
        tracing::info!("Starting claude-notify monitor in foreground mode");
        println!("🔍 Starting Claude usage monitor...");
        println!("Press Ctrl+C to stop");
        println!();
    } else {
        tracing::info!("Starting claude-notify monitor in background mode");
    }

    // Load configuration and state
    let config = Config::load()?;
    let mut state = MonitorState::load()?;

    let poll_interval = Duration::from_secs(config.general.poll_interval_seconds);

    loop {
        if let Err(e) = check_usage(&config, &mut state, foreground) {
            tracing::error!("Error checking usage: {}", e);
            if foreground {
                eprintln!("❌ Error: {}", e);
            }
        }

        // Save state after each check
        if let Err(e) = state.save() {
            tracing::error!("Error saving state: {}", e);
        }

        if foreground {
            println!("⏰ Next check in {} seconds...\n", config.general.poll_interval_seconds);
        }

        std::thread::sleep(poll_interval);
    }
}

fn check_usage(config: &Config, state: &mut MonitorState, verbose: bool) -> anyhow::Result<()> {
    if verbose {
        println!("📊 Checking usage...");
    }
    tracing::debug!("Fetching usage data");

    // Load session data
    let session = SessionData::load(&config.auth.session_file)?;

    // Convert session data to cookie pairs
    let cookie_pairs = session_to_cookie_pairs(&session);

    // Launch headless Chrome
    let browser = BrowserAuthenticator::new_headless(false)?;

    // Inject stored cookies
    browser.inject_cookies(cookie_pairs, false)?;

    // Fetch usage data
    let usage_json = browser.fetch_usage_api(&session.org_id, false)?;

    // Parse the response
    let usage: UsageResponse = serde_json::from_value(usage_json)?;

    if verbose {
        println!("  ✓ Fetched usage data");
    }

    // Create notification sender
    let notification_sender = RealNotificationSender;

    // Process 5-hour limit
    process_limit(
        &notification_sender,
        config,
        state,
        LimitType::FiveHour,
        &usage.five_hour,
        300, // 5 hours in minutes
        LimitType::SevenDay,
        &usage.seven_day,
        verbose,
    )?;

    // Process 7-day limit
    process_limit(
        &notification_sender,
        config,
        state,
        LimitType::SevenDay,
        &usage.seven_day,
        10080, // 7 days in minutes
        LimitType::FiveHour,
        &usage.five_hour,
        verbose,
    )?;

    if verbose {
        println!("  ✓ All checks complete");
    }

    Ok(())
}

fn process_limit(
    sender: &dyn NotificationSender,
    config: &Config,
    state: &mut MonitorState,
    limit_type: LimitType,
    limit: &UsageLimit,
    period_minutes: i64,
    other_limit_type: LimitType,
    other_limit: &UsageLimit,
    verbose: bool,
) -> anyhow::Result<()> {
    // API returns utilization as percentage already
    let percentage = limit.utilization;

    // Parse reset time
    let (reset_time_utc, resets_in) = match &limit.resets_at {
        Some(resets_at_str) => {
            let reset_time = DateTime::parse_from_rfc3339(resets_at_str)
                .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", resets_at_str)))?;
            let reset_time_utc: DateTime<Utc> = reset_time.into();

            // Calculate time until reset
            let now = Utc::now();
            let duration_until_reset = reset_time_utc.signed_duration_since(now);
            let resets_in_str = format_duration(duration_until_reset);

            (Some(reset_time_utc), resets_in_str)
        }
        None => {
            // API returned null for reset time - skip reset-related notifications
            tracing::warn!("{} limit has null reset time, skipping reset checks", limit_type.as_str());
            (None, "Unknown".to_string())
        }
    };

    if verbose {
        println!(
            "  {} limit: {:.1}% (resets in {})",
            limit_type.as_str(),
            percentage,
            resets_in
        );
    }

    // Check if a reset occurred (only if we have a valid reset time)
    if let Some(reset_time) = reset_time_utc {
        let reset_occurred = state.check_and_handle_reset(limit_type, reset_time);

        if reset_occurred {
            tracing::info!("{} limit has reset", limit_type.as_str());
            if verbose {
                println!("  ✨ {} limit has been reset!", limit_type.as_str());
            }

            // Send reset notification if not already sent
            if !state.is_reset_notified(limit_type) {
                notify_reset(sender, &config.notifications, limit_type)?;
                state.mark_reset_notified(limit_type);
                tracing::info!("Sent reset notification for {} limit", limit_type.as_str());
            }
        }

        // Check for upcoming reset notification
        let minutes_before_reset = match limit_type {
            LimitType::FiveHour => config.notifications.minutes_before_five_hour_reset,
            LimitType::SevenDay => config.notifications.minutes_before_seven_day_reset,
        };

        if let Some(notification_minutes) = minutes_before_reset {
            let now = Utc::now();
            let time_until_reset = reset_time.signed_duration_since(now);
            let minutes_until_reset = time_until_reset.num_minutes();

            // Check if we're within the notification window and haven't notified yet
            if minutes_until_reset > 0
                && minutes_until_reset <= notification_minutes as i64
                && !state.is_upcoming_reset_notified(limit_type)
            {
                let remaining_capacity = 100.0 - percentage;
                notify_upcoming_reset(
                    sender,
                    &config.notifications,
                    limit_type,
                    percentage,
                    remaining_capacity,
                    &resets_in,
                )?;
                state.mark_upcoming_reset_notified(limit_type);
                tracing::info!(
                    "Sent upcoming reset notification for {} limit ({} minutes before reset)",
                    limit_type.as_str(),
                    minutes_until_reset
                );

                if verbose {
                    println!(
                        "  ⏰ Sent upcoming reset notification ({} minutes until reset)",
                        minutes_until_reset
                    );
                }
            }
        }

        // Check for unused capacity warning (time + capacity threshold)
        let capacity_warning_config = match limit_type {
            LimitType::FiveHour => config.notifications.capacity_warning_five_hour,
            LimitType::SevenDay => config.notifications.capacity_warning_seven_day,
        };

        if let Some((minutes_threshold, min_capacity_pct)) = capacity_warning_config {
            let now = Utc::now();
            let time_until_reset = reset_time.signed_duration_since(now);
            let minutes_until_reset = time_until_reset.num_minutes();
            let remaining_capacity = 100.0 - percentage;

            // Check if we're within the time window AND have sufficient remaining capacity
            if minutes_until_reset > 0
                && minutes_until_reset <= minutes_threshold as i64
                && remaining_capacity >= min_capacity_pct as f64
                && !state.is_capacity_warning_notified(limit_type)
            {
                // Parse the other limit's reset time
                let other_percentage = other_limit.utilization;
                let other_reset_time = match &other_limit.resets_at {
                    Some(resets_at_str) => {
                        let reset_time = DateTime::parse_from_rfc3339(resets_at_str)
                            .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", resets_at_str)))?;
                        let reset_time_utc: DateTime<Utc> = reset_time.into();
                        reset_time_utc
                    }
                    None => {
                        // If other limit has no reset time, use a far future date
                        Utc::now() + chrono::Duration::days(365)
                    }
                };

                notify_unused_capacity(
                    sender,
                    &config.notifications,
                    limit_type,
                    percentage,
                    remaining_capacity,
                    &resets_in,
                    other_limit_type,
                    other_percentage,
                    other_reset_time,
                    &config.general.timezone,
                )?;
                state.mark_capacity_warning_notified(limit_type);
                tracing::info!(
                    "Sent unused capacity warning for {} limit ({} minutes until reset, {:.0}% capacity remaining)",
                    limit_type.as_str(),
                    minutes_until_reset,
                    remaining_capacity
                );

                if verbose {
                    println!(
                        "  💡 Sent capacity warning ({} minutes until reset, {:.0}% remaining)",
                        minutes_until_reset,
                        remaining_capacity
                    );
                }
            }
        }
    }

    // Check for percentage-based usage warning
    let percentage_warning_config = match limit_type {
        LimitType::FiveHour => config.notifications.percentage_warning_five_hour,
        LimitType::SevenDay => config.notifications.percentage_warning_seven_day,
    };

    if let Some(warn_at_percentage) = percentage_warning_config {
        if percentage >= warn_at_percentage as f64 && !state.is_percentage_warning_notified(limit_type) {
            crate::notifications::notify_percentage_warning(
                sender,
                &config.notifications,
                limit_type,
                percentage,
                warn_at_percentage,
                &resets_in,
            )?;
            state.mark_percentage_warning_notified(limit_type);
            tracing::info!(
                "Sent percentage warning for {} limit at {}%",
                limit_type.as_str(),
                percentage
            );

            if verbose {
                println!(
                    "  🔔 Sent notification: {:.1}% usage warning (threshold {}%)",
                    percentage, warn_at_percentage
                );
            }
        }
    }

    // Get thresholds for this limit type
    let thresholds = match limit_type {
        LimitType::FiveHour => &config.thresholds.five_hour,
        LimitType::SevenDay => &config.thresholds.seven_day,
    };

    // Check threshold crossings - only notify for the highest crossed threshold
    let crossed_thresholds: Vec<u8> = thresholds
        .iter()
        .filter(|&&threshold| percentage >= threshold as f64 && !state.is_threshold_notified(limit_type, threshold))
        .copied()
        .collect();

    if let Some(&highest_threshold) = crossed_thresholds.iter().max() {
        notify_threshold(
            sender,
            &config.notifications,
            limit_type,
            percentage,
            &resets_in,
        )?;
        state.mark_threshold_notified(limit_type, highest_threshold);
        tracing::info!(
            "Sent threshold notification for {} limit at {}%",
            limit_type.as_str(),
            highest_threshold
        );

        if verbose {
            println!(
                "  🔔 Sent notification: {}% threshold crossed",
                highest_threshold
            );
        }
    }

    // Check for predicted overage (only for 5-hour limit, and only if we're past 50%)
    // Also requires valid reset time for prediction
    if limit_type == LimitType::FiveHour && percentage >= 50.0 {
        if let Some(reset_time) = reset_time_utc {
            let now = Utc::now();
            let period_start = reset_time - chrono::Duration::minutes(period_minutes);
            let elapsed_duration = now.signed_duration_since(period_start);
            let elapsed_minutes = elapsed_duration.num_minutes().max(1); // Avoid division by zero
            let time_percentage = (elapsed_minutes as f64 / period_minutes as f64 * 100.0)
                .max(0.0)
                .min(100.0);

            // Only predict if we have some meaningful time elapsed
            if time_percentage > 10.0 {
                let usage_rate = percentage / time_percentage; // usage per 1% of time
                let predicted_percentage = usage_rate * 100.0;

                // Warn if predicted to exceed 100% and we haven't warned recently
                if predicted_percentage > 100.0 && state.should_warn_overage(limit_type) {
                    notify_predicted_overage(
                        sender,
                        &config.notifications,
                        limit_type,
                        percentage,
                        predicted_percentage,
                        &resets_in,
                    )?;
                    state.mark_overage_warned(limit_type);
                    tracing::info!(
                        "Sent overage prediction for {} limit: predicted {:.0}%",
                        limit_type.as_str(),
                        predicted_percentage
                    );

                    if verbose {
                        println!(
                            "  ⚡ Sent overage warning: predicted {:.0}% usage",
                            predicted_percentage
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn session_to_cookie_pairs(session: &SessionData) -> Vec<(String, String)> {
    let mut cookies = vec![("sessionKey".to_string(), session.session_key.clone())];

    if let Some(ref cf) = session.cf_clearance {
        cookies.push(("cf_clearance".to_string(), cf.clone()));
    }
    if let Some(ref org) = session.last_active_org {
        cookies.push(("lastActiveOrg".to_string(), org.clone()));
    }
    if let Some(ref device) = session.anthropic_device_id {
        cookies.push(("anthropic-device-id".to_string(), device.clone()));
    }
    if let Some(ref bm) = session.cf_bm {
        cookies.push(("__cf_bm".to_string(), bm.clone()));
    }
    if let Some(ref ssid) = session.ssid {
        cookies.push(("__ssid".to_string(), ssid.clone()));
    }

    cookies
}
