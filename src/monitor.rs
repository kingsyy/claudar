use crate::browser_auth::{fetch_usage_direct, BrowserAuthenticator};
use crate::config::Config;
use crate::notification_trait::{NotificationSender, RealNotificationSender};
use crate::notifications::{
    format_duration, notify_predicted_overage, notify_reset, notify_threshold,
    notify_upcoming_reset, notify_unused_capacity,
};
use crate::state::{LimitType, MonitorState};
use crate::storage::SessionData;
use crate::time_format;
use chrono::{DateTime, Local, Utc};
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

fn get_current_time_formatted(tz: &str) -> String {
    let now = Local::now();
    time_format::format_datetime_24h(&now, tz)
        .unwrap_or_else(|_| now.format("%H:%M").to_string())
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

    // Load configuration
    let config = Config::load()?;
    let instances = config.effective_instances();

    if foreground && instances.len() > 1 {
        println!("Monitoring {} instances: {}", instances.len(),
            instances.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(", "));
        println!();
    }

    let poll_interval = Duration::from_secs(config.general.poll_interval_seconds);

    // Browser is kept as a lazy fallback only — used when direct HTTP fetch is blocked
    // by Cloudflare. Direct fetch avoids spawning Chrome entirely, which prevents macOS
    // from accumulating ~1.3GB code-sign clones on every daemon restart.
    let mut fallback_browser: Option<BrowserAuthenticator> = None;

    loop {
        for instance in &instances {
            let instance_name = &instance.name;
            let mut state = MonitorState::load_for(&config, instance_name)?;

            if foreground && instances.len() > 1 {
                let timestamp = get_current_time_formatted(&config.general.timezone);
                println!("[{}] ── {} (periodic poll) ──", timestamp, instance_name);
            } else if foreground {
                let timestamp = get_current_time_formatted(&config.general.timezone);
                println!("[{}] 📋 Periodic poll: {}", timestamp, instance_name);
            }

            if let Err(e) = check_usage(&mut fallback_browser, &config, &mut state, instance_name, foreground, &config.general.timezone) {
                tracing::error!("Error checking usage for instance '{}': {}", instance_name, e);
                if foreground {
                    let timestamp = get_current_time_formatted(&config.general.timezone);
                    eprintln!("[{}] ❌ Error ({}): {}", timestamp, instance_name, e);
                }

                // Drop the browser on WebSocket/connection errors so it is recreated next cycle.
                let err_msg = e.to_string();
                if err_msg.contains("connection") || err_msg.contains("closed") || err_msg.contains("WebSocket") {
                    tracing::warn!("Browser connection lost, will recreate on next poll");
                    fallback_browser = None;
                }
            }

            if let Err(e) = state.save_for(&config, instance_name) {
                tracing::error!("Error saving state for instance '{}': {}", instance_name, e);
            }
        }

        if foreground {
            let timestamp = get_current_time_formatted(&config.general.timezone);
            println!("[{}] ⏰ Next check in {} seconds...\n", timestamp, config.general.poll_interval_seconds);
        }

        std::thread::sleep(poll_interval);
    }
}

fn check_usage(
    fallback_browser: &mut Option<BrowserAuthenticator>,
    config: &Config,
    state: &mut MonitorState,
    instance_name: &str,
    verbose: bool,
    timezone: &str,
) -> anyhow::Result<()> {
    if verbose {
        let timestamp = get_current_time_formatted(timezone);
        println!("[{}] 📊 Checking usage...", timestamp);
    }
    tracing::debug!("Fetching usage data for instance '{}'", instance_name);

    let session_path = config.session_path_for(instance_name)?;
    let session = SessionData::load(&session_path)?;
    let cookie_pairs = session.cookie_pairs();

    // Try a direct HTTP request first — no Chrome process, no code-sign clones.
    let usage_json = match fetch_usage_direct(&cookie_pairs, &session.org_id, verbose) {
        Ok(json) => {
            tracing::info!("monitor: direct fetch succeeded for '{}'", instance_name);
            json
        }
        Err(e) => {
            let err_msg = e.to_string();
            let needs_browser = err_msg.starts_with("cloudflare_challenge")
                || err_msg.starts_with("html_response")
                || err_msg.starts_with("auth_error");

            if needs_browser {
                tracing::warn!(
                    "monitor: direct fetch blocked for '{}' ({}), switching to browser fallback",
                    instance_name,
                    err_msg
                );
                if verbose {
                    let timestamp = get_current_time_formatted(timezone);
                    println!("[{}]   ⚠ Direct fetch blocked, falling back to headless browser...", timestamp);
                }

                // Lazily create the fallback browser only when actually needed.
                if fallback_browser.is_none() {
                    let profile = Config::config_dir()?.join("chrome-profiles").join("monitor");
                    tracing::info!("monitor: launching fallback headless browser (profile: {:?})", profile);
                    *fallback_browser = Some(BrowserAuthenticator::new_headless(&profile, verbose)?);
                }

                let browser = fallback_browser.as_ref().unwrap();
                browser.inject_cookies(cookie_pairs, verbose)?;
                let json = browser.fetch_usage_api(&session.org_id, verbose)?;
                tracing::info!("monitor: browser fallback succeeded for '{}'", instance_name);
                json
            } else {
                // Network error, JSON parse failure, etc. — propagate as-is.
                return Err(e);
            }
        }
    };

    let usage: UsageResponse = serde_json::from_value(usage_json)?;

    if verbose {
        let timestamp = get_current_time_formatted(timezone);
        println!("[{}]   ✓ Fetched usage data", timestamp);
    }

    // Create notification sender
    let notification_sender = RealNotificationSender;

    // Process 5-hour limit
    process_limit(
        &notification_sender,
        config,
        state,
        instance_name,
        LimitType::FiveHour,
        &usage.five_hour,
        300, // 5 hours in minutes
        LimitType::SevenDay,
        &usage.seven_day,
        verbose,
        timezone,
    )?;

    // Process 7-day limit
    process_limit(
        &notification_sender,
        config,
        state,
        instance_name,
        LimitType::SevenDay,
        &usage.seven_day,
        10080, // 7 days in minutes
        LimitType::FiveHour,
        &usage.five_hour,
        verbose,
        timezone,
    )?;

    if verbose {
        let timestamp = get_current_time_formatted(timezone);
        println!("[{}]   ✓ All checks complete", timestamp);
    }

    Ok(())
}

fn process_limit(
    sender: &dyn NotificationSender,
    config: &Config,
    state: &mut MonitorState,
    instance_name: &str,
    limit_type: LimitType,
    limit: &UsageLimit,
    period_minutes: i64,
    other_limit_type: LimitType,
    other_limit: &UsageLimit,
    verbose: bool,
    timezone: &str,
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
        let timestamp = get_current_time_formatted(timezone);
        println!(
            "[{}]   {} limit: {:.1}% (resets in {})",
            timestamp,
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
                let timestamp = get_current_time_formatted(timezone);
                println!("[{}]   ✨ {} limit has been reset!", timestamp, limit_type.as_str());
            }

            // Send reset notification if not already sent
            if !state.is_reset_notified(limit_type) {
                notify_reset(sender, &config.notifications, instance_name, limit_type)?;
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
                    instance_name,
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
                    let timestamp = get_current_time_formatted(timezone);
                    println!(
                        "[{}]   ⏰ Sent upcoming reset notification ({} minutes until reset)",
                        timestamp,
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
                    instance_name,
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
                    let timestamp = get_current_time_formatted(timezone);
                    println!(
                        "[{}]   💡 Sent capacity warning ({} minutes until reset, {:.0}% remaining)",
                        timestamp,
                        minutes_until_reset,
                        remaining_capacity
                    );
                }
            }
        }
    }

    // Compute predicted end-of-period percentage (if we have a reset time and enough elapsed time)
    let predicted_percentage: Option<f64> = if let Some(reset_time) = reset_time_utc {
        let now = Utc::now();
        let period_start = reset_time - chrono::Duration::minutes(period_minutes);
        let elapsed_minutes = now.signed_duration_since(period_start).num_minutes().max(1);
        let time_pct = (elapsed_minutes as f64 / period_minutes as f64 * 100.0).min(100.0);
        if time_pct > 10.0 {
            Some((percentage / time_pct) * 100.0)
        } else {
            None
        }
    } else {
        None
    };

    // Get thresholds for this limit type
    let thresholds = match limit_type {
        LimitType::FiveHour => &config.thresholds.five_hour,
        LimitType::SevenDay => &config.thresholds.seven_day,
    };

    // Ensure 100% is always checked as a threshold
    let mut thresholds_to_check = thresholds.clone();
    if !thresholds_to_check.contains(&100) {
        thresholds_to_check.push(100);
    }

    // Check threshold crossings - only notify for the highest crossed threshold
    let crossed_thresholds: Vec<u8> = thresholds_to_check
        .iter()
        .filter(|&&threshold| percentage >= threshold as f64 && !state.is_threshold_notified(limit_type, threshold))
        .copied()
        .collect();

    if let Some(&highest_threshold) = crossed_thresholds.iter().max() {
        notify_threshold(
            sender,
            &config.notifications,
            instance_name,
            limit_type,
            percentage,
            &resets_in,
            predicted_percentage,
            reset_time_utc,
            period_minutes,
        )?;
        state.mark_threshold_notified(limit_type, highest_threshold);
        tracing::info!(
            "Sent threshold notification for {} limit at {}%",
            limit_type.as_str(),
            highest_threshold
        );

        if verbose {
            let timestamp = get_current_time_formatted(timezone);
            println!(
                "[{}]   🔔 Sent notification: {}% threshold crossed",
                timestamp,
                highest_threshold
            );
        }
    }

    // Check for predicted overage (only for 5-hour limit, and only if we're past 50%)
    if limit_type == LimitType::FiveHour && percentage >= 50.0 {
        if let Some(pred) = predicted_percentage {
            // Warn if predicted to exceed 100% and we haven't warned recently
            if pred > 100.0 && state.should_warn_overage(limit_type) {
                notify_predicted_overage(
                    sender,
                    &config.notifications,
                    instance_name,
                    limit_type,
                    percentage,
                    pred,
                    &resets_in,
                    reset_time_utc,
                    period_minutes,
                )?;
                state.mark_overage_warned(limit_type);
                tracing::info!(
                    "Sent overage prediction for {} limit: predicted {:.0}%",
                    limit_type.as_str(),
                    pred
                );

                if verbose {
                    let timestamp = get_current_time_formatted(timezone);
                    println!(
                        "[{}]   ⚡ Sent overage warning: predicted {:.0}% usage",
                        timestamp,
                        pred
                    );
                }
            }
        }
    }

    Ok(())
}

