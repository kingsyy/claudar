use crate::config::Config;
use crate::history::{self, HistoryRecord};
use crate::notification_trait::{NotificationSender, RealNotificationSender};
use crate::notifications::{
    format_duration, notify_predicted_overage, notify_reset, notify_threshold,
    notify_upcoming_reset, notify_unused_capacity,
};
use crate::state::{LimitType, MonitorState};
use crate::storage::SessionData;
use crate::time_format;
use crate::usage_fetcher::{AuthRequiredError, UsageLimit, UsageResponse, fetch_usage};
use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use std::time::Duration;

/// Usage snapshot emitted as a Tauri `usage-update` event after each successful poll.
#[derive(Debug, Clone, Serialize)]
pub struct UsagePayload {
    pub instance: String,
    pub five_hour_pct: f64,
    pub seven_day_pct: f64,
    /// Five-hour reset timestamp (raw ISO 8601 string from the API).
    pub resets_at: Option<String>,
    /// Seven-day reset timestamp.
    pub seven_day_resets_at: Option<String>,
    /// Linear-extrapolation of 5-hour usage to end-of-period (`None` if < 10% elapsed).
    pub predicted_pct: Option<f64>,
}

/// Run one full poll cycle for a single instance and return a `UsagePayload`.
///
/// Loads session cookies, fetches the Claude.ai usage API, fires desktop
/// notifications for any crossed thresholds, persists state, and always
/// appends a `HistoryRecord` (regardless of `config.history.enabled`).
///
/// Returns `Err` containing `AuthRequiredError` when the session is expired or
/// Cloudflare-blocked; the caller should emit `auth-required` and skip the cycle.
pub async fn poll_instance(
    config: &Config,
    instance_name: &str,
    sender: &dyn NotificationSender,
) -> anyhow::Result<UsagePayload> {
    let mut state = MonitorState::load_for(config, instance_name)?;
    let result = check_usage(config, &mut state, instance_name, false, &config.general.timezone, sender).await;
    // Always persist state updates, even when a fetch error occurred mid-way.
    let _ = state.save_for(config, instance_name);
    let payload = result?;

    // GUI always records history regardless of config.history.enabled.
    let record = HistoryRecord {
        polled_at: Utc::now(),
        five_hour_pct: payload.five_hour_pct,
        five_hour_resets_at: payload.resets_at.clone(),
        seven_day_pct: payload.seven_day_pct,
        seven_day_resets_at: payload.seven_day_resets_at.clone(),
        five_hour_predicted_pct: payload.predicted_pct,
    };
    if let Err(e) = history::append_record(config, instance_name, &record) {
        tracing::warn!("Failed to write history for '{}': {}", instance_name, e);
    }

    Ok(payload)
}

fn get_current_time_formatted(tz: &str) -> String {
    let now = Local::now();
    time_format::format_datetime_24h(&now, tz)
        .unwrap_or_else(|_| now.format("%H:%M").to_string())
}

fn get_current_time_formatted_full(tz: &str) -> String {
    let now = Local::now();
    let tz_lower = tz.to_lowercase();

    match tz_lower.as_str() {
        "local" => {
            let local_dt = now.with_timezone(&Local);
            local_dt.format("%H:%M %d/%m/%Y").to_string()
        }
        "utc" => {
            let utc_dt = now.with_timezone(&chrono::Utc);
            utc_dt.format("%H:%M %d/%m/%Y").to_string()
        }
        _ => {
            use std::str::FromStr;
            if let Ok(tz_parsed) = chrono_tz::Tz::from_str(tz) {
                let converted_dt = now.with_timezone(&tz_parsed);
                converted_dt.format("%H:%M %d/%m/%Y").to_string()
            } else {
                now.format("%H:%M %d/%m/%Y").to_string()
            }
        }
    }
}

pub async fn run_monitor(foreground: bool) -> anyhow::Result<()> {
    if foreground {
        tracing::info!("Starting claude-notify monitor in foreground mode");
    } else {
        tracing::info!("Starting claude-notify monitor in background mode");
    }

    let config = Config::load()?;
    let instances = config.effective_instances();
    let sender = RealNotificationSender;

    if foreground && instances.len() > 1 {
        tracing::info!(
            "Monitoring {} instances: {}",
            instances.len(),
            instances
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let poll_interval = Duration::from_secs(config.general.poll_interval_seconds);

    loop {
        for instance in &instances {
            let instance_name = &instance.name;
            let mut state = MonitorState::load_for(&config, instance_name)?;

            if foreground && instances.len() > 1 {
                let timestamp = get_current_time_formatted_full(&config.general.timezone);
                tracing::info!("── {} ── {} ──", instance_name, timestamp);
            } else if foreground {
                let timestamp = get_current_time_formatted_full(&config.general.timezone);
                tracing::info!("{} [{}]", instance_name, timestamp);
            }

            match check_usage(&config, &mut state, instance_name, foreground, &config.general.timezone, &sender).await {
                Ok(_payload) => {}
                Err(e) => {
                    // AuthRequiredError means session expired or Cloudflare blocked — log
                    // and continue the monitor loop rather than crashing.
                    if e.downcast_ref::<AuthRequiredError>().is_some() {
                        tracing::warn!(
                            "monitor: auth required for '{}' — run setup to refresh session: {}",
                            instance_name,
                            e
                        );
                    } else {
                        tracing::error!(
                            "Error checking usage for instance '{}': {}",
                            instance_name,
                            e
                        );
                    }
                }
            }

            if let Err(e) = state.save_for(&config, instance_name) {
                tracing::error!(
                    "Error saving state for instance '{}': {}",
                    instance_name,
                    e
                );
            }
        }

        if foreground {
            let timestamp = get_current_time_formatted(&config.general.timezone);
            tracing::info!(
                "[{}] Next check in {} seconds...",
                timestamp,
                config.general.poll_interval_seconds
            );
        }

        tokio::time::sleep(poll_interval).await;
    }
}

/// Compute the linear-extrapolation prediction of usage at end-of-period.
/// Returns `None` when there is no reset time or less than 10% of the period has elapsed.
fn compute_predicted_pct(
    utilization: f64,
    resets_at: &Option<String>,
    period_minutes: i64,
) -> Option<f64> {
    let resets_at_str = resets_at.as_deref()?;
    let reset_time = DateTime::parse_from_rfc3339(resets_at_str)
        .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", resets_at_str)))
        .ok()?;
    let reset_time_utc: DateTime<Utc> = reset_time.into();
    let now = Utc::now();
    let period_start = reset_time_utc - chrono::Duration::minutes(period_minutes);
    let elapsed_minutes = now.signed_duration_since(period_start).num_minutes().max(1);
    let time_pct = (elapsed_minutes as f64 / period_minutes as f64 * 100.0).min(100.0);
    if time_pct > 10.0 {
        Some((utilization / time_pct) * 100.0)
    } else {
        None
    }
}

async fn check_usage(
    config: &Config,
    state: &mut MonitorState,
    instance_name: &str,
    verbose: bool,
    timezone: &str,
    sender: &dyn NotificationSender,
) -> anyhow::Result<UsagePayload> {
    if verbose {
        let timestamp = get_current_time_formatted(timezone);
        tracing::info!("[{}] Checking usage...", timestamp);
    }
    tracing::debug!("Fetching usage data for instance '{}'", instance_name);

    let session_path = config.session_path_for(instance_name)?;
    let session = SessionData::load(&session_path)?;
    let cookie_header = session.cookie_header_string();

    tracing::debug!(
        "usage_fetch: sending {} cookie entries for '{}'",
        cookie_header.split(';').count(),
        instance_name
    );

    let usage_json = fetch_usage(&cookie_header, &session.org_id).await?;
    let usage: UsageResponse = serde_json::from_value(usage_json)?;

    if verbose {
        let timestamp = get_current_time_formatted(timezone);
        tracing::info!("[{}] Fetched usage data", timestamp);
    }

    process_limit(
        sender,
        config,
        state,
        instance_name,
        LimitType::FiveHour,
        &usage.five_hour,
        300,
        LimitType::SevenDay,
        &usage.seven_day,
        verbose,
        timezone,
    )?;

    process_limit(
        sender,
        config,
        state,
        instance_name,
        LimitType::SevenDay,
        &usage.seven_day,
        10080,
        LimitType::FiveHour,
        &usage.five_hour,
        verbose,
        timezone,
    )?;

    if verbose {
        let timestamp = get_current_time_formatted(timezone);
        tracing::info!("[{}] All checks complete", timestamp);
    }

    // CLI path: respect config.history.enabled for history writing.
    if config.history.enabled {
        let record = HistoryRecord {
            polled_at: Utc::now(),
            five_hour_pct: usage.five_hour.utilization,
            five_hour_resets_at: usage.five_hour.resets_at.clone(),
            seven_day_pct: usage.seven_day.utilization,
            seven_day_resets_at: usage.seven_day.resets_at.clone(),
            five_hour_predicted_pct: compute_predicted_pct(
                usage.five_hour.utilization,
                &usage.five_hour.resets_at,
                300,
            ),
        };
        if let Err(e) = history::append_record(config, instance_name, &record) {
            tracing::warn!(
                "Failed to write history record for '{}': {}",
                instance_name,
                e
            );
        }
    }

    let predicted_pct = compute_predicted_pct(
        usage.five_hour.utilization,
        &usage.five_hour.resets_at,
        300,
    );

    Ok(UsagePayload {
        instance: instance_name.to_string(),
        five_hour_pct: usage.five_hour.utilization,
        seven_day_pct: usage.seven_day.utilization,
        resets_at: usage.five_hour.resets_at.clone(),
        seven_day_resets_at: usage.seven_day.resets_at.clone(),
        predicted_pct,
    })
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
    let percentage = limit.utilization;

    let (reset_time_utc, resets_in) = match &limit.resets_at {
        Some(resets_at_str) => {
            let reset_time = DateTime::parse_from_rfc3339(resets_at_str)
                .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", resets_at_str)))?;
            let reset_time_utc: DateTime<Utc> = reset_time.into();

            let now = Utc::now();
            let duration_until_reset = reset_time_utc.signed_duration_since(now);
            let resets_in_str = format_duration(duration_until_reset);

            (Some(reset_time_utc), resets_in_str)
        }
        None => {
            tracing::warn!(
                "{} limit has null reset time, skipping reset checks",
                limit_type.as_str()
            );
            (None, "Unknown".to_string())
        }
    };

    if verbose {
        let timestamp = get_current_time_formatted(timezone);
        tracing::info!(
            "[{}] {} limit: {:.1}% (resets in {})",
            timestamp,
            limit_type.as_str(),
            percentage,
            resets_in
        );
    }

    if let Some(reset_time) = reset_time_utc {
        let reset_occurred = state.check_and_handle_reset(limit_type, reset_time);

        if reset_occurred {
            tracing::info!("{} limit has reset", limit_type.as_str());

            if !state.is_reset_notified(limit_type) {
                notify_reset(sender, &config.notifications, instance_name, limit_type)?;
                state.mark_reset_notified(limit_type);
                tracing::info!("Sent reset notification for {} limit", limit_type.as_str());
            }
        }

        let minutes_before_reset = match limit_type {
            LimitType::FiveHour => config.notifications.minutes_before_five_hour_reset,
            LimitType::SevenDay => config.notifications.minutes_before_seven_day_reset,
        };

        if let Some(notification_minutes) = minutes_before_reset {
            let now = Utc::now();
            let time_until_reset = reset_time.signed_duration_since(now);
            let minutes_until_reset = time_until_reset.num_minutes();

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
            }
        }

        let capacity_warning_config = match limit_type {
            LimitType::FiveHour => config.notifications.capacity_warning_five_hour,
            LimitType::SevenDay => config.notifications.capacity_warning_seven_day,
        };

        if let Some((minutes_threshold, min_capacity_pct)) = capacity_warning_config {
            let now = Utc::now();
            let time_until_reset = reset_time.signed_duration_since(now);
            let minutes_until_reset = time_until_reset.num_minutes();
            let remaining_capacity = 100.0 - percentage;

            if minutes_until_reset > 0
                && minutes_until_reset <= minutes_threshold as i64
                && remaining_capacity >= min_capacity_pct as f64
                && !state.is_capacity_warning_notified(limit_type)
            {
                let other_percentage = other_limit.utilization;
                let other_reset_time = match &other_limit.resets_at {
                    Some(resets_at_str) => {
                        let reset_time = DateTime::parse_from_rfc3339(resets_at_str)
                            .or_else(|_| {
                                DateTime::parse_from_rfc3339(&format!("{}Z", resets_at_str))
                            })?;
                        let reset_time_utc: DateTime<Utc> = reset_time.into();
                        reset_time_utc
                    }
                    None => Utc::now() + chrono::Duration::days(365),
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
            }
        }
    }

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

    let thresholds = match limit_type {
        LimitType::FiveHour => &config.thresholds.five_hour,
        LimitType::SevenDay => &config.thresholds.seven_day,
    };

    let mut thresholds_to_check = thresholds.clone();
    if !thresholds_to_check.contains(&100) {
        thresholds_to_check.push(100);
    }

    let crossed_thresholds: Vec<u8> = thresholds_to_check
        .iter()
        .filter(|&&threshold| {
            percentage >= threshold as f64 && !state.is_threshold_notified(limit_type, threshold)
        })
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
    }

    if limit_type == LimitType::FiveHour && percentage >= 50.0 {
        if let Some(pred) = predicted_percentage {
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
            }
        }
    }

    Ok(())
}
