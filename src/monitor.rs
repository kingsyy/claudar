use crate::browser_auth::BrowserAuthenticator;
use crate::config::Config;
use crate::notifications::{format_duration, notify_predicted_overage, notify_reset, notify_threshold};
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
    resets_at: String,
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

    // Process 5-hour limit
    process_limit(
        config,
        state,
        LimitType::FiveHour,
        &usage.five_hour,
        300, // 5 hours in minutes
        verbose,
    )?;

    // Process 7-day limit
    process_limit(
        config,
        state,
        LimitType::SevenDay,
        &usage.seven_day,
        10080, // 7 days in minutes
        verbose,
    )?;

    if verbose {
        println!("  ✓ All checks complete");
    }

    Ok(())
}

fn process_limit(
    config: &Config,
    state: &mut MonitorState,
    limit_type: LimitType,
    limit: &UsageLimit,
    period_minutes: i64,
    verbose: bool,
) -> anyhow::Result<()> {
    // Convert utilization to percentage
    let percentage = if limit.utilization <= 1.0 {
        limit.utilization * 100.0
    } else {
        limit.utilization
    };

    // Parse reset time
    let reset_time = DateTime::parse_from_rfc3339(&limit.resets_at)
        .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", &limit.resets_at)))?;
    let reset_time_utc: DateTime<Utc> = reset_time.into();

    // Calculate time until reset
    let now = Utc::now();
    let duration_until_reset = reset_time_utc.signed_duration_since(now);
    let resets_in = format_duration(duration_until_reset);

    if verbose {
        println!(
            "  {} limit: {:.1}% (resets in {})",
            limit_type.as_str(),
            percentage,
            resets_in
        );
    }

    // Check if a reset occurred
    let reset_occurred = state.check_and_handle_reset(limit_type, reset_time_utc);

    if reset_occurred {
        tracing::info!("{} limit has reset", limit_type.as_str());
        if verbose {
            println!("  ✨ {} limit has been reset!", limit_type.as_str());
        }

        // Send reset notification if not already sent
        if !state.is_reset_notified(limit_type) {
            notify_reset(&config.notifications, limit_type)?;
            state.mark_reset_notified(limit_type);
            tracing::info!("Sent reset notification for {} limit", limit_type.as_str());
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
    if limit_type == LimitType::FiveHour && percentage >= 50.0 {
        let period_start = reset_time_utc - chrono::Duration::minutes(period_minutes);
        let elapsed_duration = now.signed_duration_since(period_start);
        let elapsed_minutes = elapsed_duration.num_minutes().max(1); // Avoid division by zero
        let time_percentage = (elapsed_minutes as f64 / period_minutes as f64 * 100.0)
            .min(100.0)
            .max(0.0);

        // Only predict if we have some meaningful time elapsed
        if time_percentage > 10.0 {
            let usage_rate = percentage / time_percentage; // usage per 1% of time
            let predicted_percentage = usage_rate * 100.0;

            // Warn if predicted to exceed 100% and we haven't warned recently
            if predicted_percentage > 100.0 && state.should_warn_overage(limit_type) {
                notify_predicted_overage(
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
