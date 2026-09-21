use crate::config::{Config, Provider};
use crate::history::{self, HistoryRecord};
use crate::notification_trait::{NotificationSender, RealNotificationSender};
use crate::notifications::{
    format_duration, notify_predicted_overage, notify_reset, notify_threshold,
    notify_upcoming_reset, notify_unused_capacity,
};
use crate::openai_fetcher;
use crate::openai_session::OpenAiSession;
use crate::state::{LimitType, MonitorState};
use crate::storage::SessionData;
use crate::time_format;
use crate::usage_fetcher::{AuthRequiredError, HttpFetcher, UsageLimit, UsageResponse, fetch_usage};
use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use std::time::Duration;

/// Usage snapshot emitted as a Tauri `usage-update` event after each successful poll.
#[derive(Debug, Clone, Serialize)]
pub struct UsagePayload {
    pub instance: String,
    /// Which service this snapshot came from, so the dashboard knows which bars
    /// are meaningful. Defaults to Claude for payloads built before providers.
    #[serde(default)]
    pub provider: Provider,
    pub five_hour_pct: f64,
    pub seven_day_pct: f64,
    /// Five-hour reset timestamp (raw ISO 8601 string from the API).
    pub resets_at: Option<String>,
    /// Seven-day reset timestamp.
    pub seven_day_resets_at: Option<String>,
    /// Linear-extrapolation of 5-hour usage to end-of-period (`None` if < 10% elapsed).
    pub predicted_pct: Option<f64>,
    /// Length of the short window in seconds, as reported by the provider.
    /// `None` means the provider reported no short window at all — a real state
    /// for a ChatGPT account, and the signal for the UI not to draw a 0% bar.
    /// Claude always reports both, so it always fills these in.
    #[serde(default)]
    pub five_hour_window_seconds: Option<i64>,
    /// Length of the long window in seconds.
    #[serde(default)]
    pub seven_day_window_seconds: Option<i64>,
}

impl UsagePayload {
    /// Whether the short window carries real data. Claude always reports both
    /// windows; a ChatGPT account may report only its weekly one, and callers
    /// must not present the resulting 0% as a measurement.
    pub fn has_short_window(&self) -> bool {
        self.provider == Provider::ClaudeWeb || self.five_hour_window_seconds.is_some()
    }
}

/// Seconds in Claude's fixed windows. ChatGPT reports its own durations.
const FIVE_HOUR_SECONDS: i64 = 5 * 60 * 60;
const SEVEN_DAY_SECONDS: i64 = 7 * 24 * 60 * 60;

/// Build the history record for a completed poll. Both callers (the GUI, which
/// always records, and the CLI, which honours `history.enabled`) go through
/// this so the two paths cannot drift.
fn history_record(payload: &UsagePayload) -> HistoryRecord {
    HistoryRecord {
        polled_at: Utc::now(),
        five_hour_pct: payload.five_hour_pct,
        five_hour_resets_at: payload.resets_at.clone(),
        seven_day_pct: payload.seven_day_pct,
        seven_day_resets_at: payload.seven_day_resets_at.clone(),
        five_hour_predicted_pct: payload.predicted_pct,
    }
}

/// Poll a ChatGPT account: mint a bearer from the stored cookie if needed, fetch
/// `/wham/usage`, and evaluate its windows.
///
/// ChatGPT reports the same shape as Claude (percent used of a rolling window
/// plus a reset timestamp), so thresholds, reset detection and pace prediction
/// all run through the same `process_limit` state machine rather than a parallel
/// one. Periods come from `limit_window_seconds` instead of being hardcoded, so
/// a plan with different window lengths still paces correctly.
/// Map ChatGPT's windows onto the pair Claudar models — shortest to the 5-hour
/// slot, longest to the 7-day slot — and compute the same pace prediction the
/// Claude path does, against the window length the API actually reported.
fn openai_payload(instance_name: &str, usage: &openai_fetcher::OpenAiUsage) -> UsagePayload {
    let predicted_pct = usage.short.as_ref().and_then(|w| {
        compute_predicted_pct(w.pct, &w.resets_at, (w.window_seconds / 60).max(1))
    });

    UsagePayload {
        instance: instance_name.to_string(),
        provider: Provider::OpenaiWeb,
        five_hour_pct: usage.short.as_ref().map(|w| w.pct).unwrap_or(0.0),
        seven_day_pct: usage.long.pct,
        resets_at: usage.short.as_ref().and_then(|w| w.resets_at.clone()),
        seven_day_resets_at: usage.long.resets_at.clone(),
        predicted_pct,
        five_hour_window_seconds: usage.short.as_ref().map(|w| w.window_seconds),
        seven_day_window_seconds: Some(usage.long.window_seconds),
    }
}

async fn check_openai_usage(
    config: &Config,
    state: &mut MonitorState,
    instance_name: &str,
    verbose: bool,
    sender: &dyn NotificationSender,
) -> anyhow::Result<UsagePayload> {
    let session_path = config.session_path_for(instance_name)?;
    let mut session = OpenAiSession::load(&session_path)?;

    let (bearer, minted) = session.ensure_bearer(Utc::now().timestamp()).await?;
    if minted {
        // Persist the refreshed bearer so the next poll skips the exchange.
        if let Err(e) = session.save(&session_path) {
            tracing::warn!("Failed to persist ChatGPT bearer for '{}': {}", instance_name, e);
        }
    }

    let usage = openai_fetcher::fetch_usage(&bearer).await?.ok_or_else(|| {
        anyhow::anyhow!("ChatGPT account '{}' reported no usage windows", instance_name)
    })?;

    let payload = openai_payload(instance_name, &usage);
    let seven_day = UsageLimit {
        utilization: payload.seven_day_pct,
        resets_at: payload.seven_day_resets_at.clone(),
    };
    let five_hour = payload.has_short_window().then(|| UsageLimit {
        utilization: payload.five_hour_pct,
        resets_at: payload.resets_at.clone(),
    });

    if let (Some(limit), Some(seconds)) = (five_hour.as_ref(), payload.five_hour_window_seconds) {
        process_limit(
            sender,
            config,
            state,
            instance_name,
            Provider::OpenaiWeb,
            LimitType::FiveHour,
            limit,
            (seconds / 60).max(1),
            LimitType::SevenDay,
            &seven_day,
            verbose,
            &config.general.timezone,
        )?;
    }

    process_limit(
        sender,
        config,
        state,
        instance_name,
        Provider::OpenaiWeb,
        LimitType::SevenDay,
        &seven_day,
        (usage.long.window_seconds / 60).max(1),
        LimitType::FiveHour,
        // An account with only a weekly window has no counterpart to compare
        // against; the unused-capacity warning it feeds is opt-in and off by
        // default, so referring back to the same window is the honest stand-in
        // rather than inventing a 0% short window that was never reported.
        five_hour.as_ref().unwrap_or(&seven_day),
        verbose,
        &config.general.timezone,
    )?;

    Ok(payload)
}

/// Run one poll cycle for a single instance: fetch, notify, persist state.
///
/// Dispatches on the instance's provider, and always saves state — even when the
/// fetch failed part-way — so notification bookkeeping survives an error.
/// History is left to the caller, because the GUI records unconditionally while
/// the CLI honours `config.history.enabled`.
async fn poll_once(
    config: &Config,
    instance_name: &str,
    sender: &dyn NotificationSender,
    http_fallback: Option<&HttpFetcher>,
    verbose: bool,
) -> anyhow::Result<UsagePayload> {
    let provider = config
        .effective_instances()
        .iter()
        .find(|i| i.name == instance_name)
        .map(|i| i.provider)
        .unwrap_or_default();

    let mut state = MonitorState::load_for(config, instance_name)?;
    let result = match provider {
        Provider::OpenaiWeb => {
            check_openai_usage(config, &mut state, instance_name, verbose, sender).await
        }
        Provider::ClaudeWeb => {
            check_usage(
                config,
                &mut state,
                instance_name,
                verbose,
                &config.general.timezone,
                sender,
                http_fallback,
            )
            .await
        }
    };
    // Always persist state updates, even when a fetch error occurred mid-way.
    let _ = state.save_for(config, instance_name);
    result
}

/// Run one full poll cycle for a single instance and return a `UsagePayload`.
///
/// Fetches usage for whichever provider the instance uses, fires desktop
/// notifications for any crossed thresholds, persists state, and always appends
/// a `HistoryRecord` (regardless of `config.history.enabled`).
///
/// Returns `Err` containing `AuthRequiredError` when the session is expired or
/// Cloudflare-blocked; the caller should emit `auth-required` and skip the cycle.
pub async fn poll_instance(
    config: &Config,
    instance_name: &str,
    sender: &dyn NotificationSender,
    http_fallback: Option<&HttpFetcher>,
) -> anyhow::Result<UsagePayload> {
    let payload = poll_once(config, instance_name, sender, http_fallback, false).await?;

    // GUI always records history regardless of config.history.enabled.
    if let Err(e) = history::append_record(config, instance_name, &history_record(&payload)) {
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
        tracing::info!("Starting claudar monitor in foreground mode");
    } else {
        tracing::info!("Starting claudar monitor in background mode");
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

            if foreground && instances.len() > 1 {
                let timestamp = get_current_time_formatted_full(&config.general.timezone);
                tracing::info!("── {} ── {} ──", instance_name, timestamp);
            } else if foreground {
                let timestamp = get_current_time_formatted_full(&config.general.timezone);
                tracing::info!("{} [{}]", instance_name, timestamp);
            }

            match poll_once(&config, instance_name, &sender, None, foreground).await {
                Ok(payload) => {
                    // CLI path: history is opt-in.
                    if config.history.enabled {
                        if let Err(e) =
                            history::append_record(&config, instance_name, &history_record(&payload))
                        {
                            tracing::warn!(
                                "Failed to write history record for '{}': {}",
                                instance_name,
                                e
                            );
                        }
                    }
                }
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
    http_fallback: Option<&HttpFetcher>,
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

    let usage_json = fetch_usage(&cookie_header, &session.org_id, http_fallback).await?;
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
        Provider::ClaudeWeb,
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
        Provider::ClaudeWeb,
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

    let predicted_pct = compute_predicted_pct(
        usage.five_hour.utilization,
        &usage.five_hour.resets_at,
        300,
    );

    Ok(UsagePayload {
        instance: instance_name.to_string(),
        provider: Provider::ClaudeWeb,
        five_hour_pct: usage.five_hour.utilization,
        seven_day_pct: usage.seven_day.utilization,
        resets_at: usage.five_hour.resets_at.clone(),
        seven_day_resets_at: usage.seven_day.resets_at.clone(),
        predicted_pct,
        five_hour_window_seconds: Some(FIVE_HOUR_SECONDS),
        seven_day_window_seconds: Some(SEVEN_DAY_SECONDS),
    })
}

fn process_limit(
    sender: &dyn NotificationSender,
    config: &Config,
    state: &mut MonitorState,
    instance_name: &str,
    provider: Provider,
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
                notify_reset(sender, &config.notifications, instance_name, provider, limit_type)?;
                state.mark_reset_notified(limit_type);
                tracing::info!("Sent reset notification for {} limit", limit_type.as_str());
            }
        }

        let minutes_before_reset = match limit_type {
            LimitType::FiveHour => config.notifications.minutes_before_five_hour_reset,
            LimitType::SevenDay => config.notifications.minutes_before_seven_day_reset,
        };

        if let Some(notification_minutes) = minutes_before_reset.filter(|_| config.notifications.notify_resets) {
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
                    provider,
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
                    provider,
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
            provider,
            limit_type,
            percentage,
            &resets_in,
            predicted_percentage,
            reset_time_utc,
            period_minutes,
        )?;
        // Mark *every* crossed band as notified, not just the highest. A single
        // poll can jump past several thresholds (e.g. 0% → 80% crosses 50 and
        // 75); we send one notification for the highest, but if we only marked
        // that one the lower bands would still count as "uncrossed" and re-fire
        // a spurious notification on the next identical poll.
        for &threshold in &crossed_thresholds {
            state.mark_threshold_notified(limit_type, threshold);
        }
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
                    provider,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification_trait::MockNotificationSender;
    use chrono::Duration;

    /// RFC3339 reset timestamp `minutes` from now (UTC, with offset).
    fn reset_in(minutes: i64) -> Option<String> {
        Some((Utc::now() + Duration::minutes(minutes)).to_rfc3339())
    }

    fn limit(util: f64, minutes: i64) -> UsageLimit {
        UsageLimit { utilization: util, resets_at: reset_in(minutes) }
    }

    // ---- compute_predicted_pct -------------------------------------------

    #[test]
    fn predicted_pct_none_without_reset_time() {
        assert_eq!(compute_predicted_pct(80.0, &None, 300), None);
    }

    #[test]
    fn predicted_pct_none_with_unparseable_reset() {
        assert_eq!(
            compute_predicted_pct(80.0, &Some("not-a-date".to_string()), 300),
            None
        );
    }

    #[test]
    fn predicted_pct_none_when_under_ten_percent_elapsed() {
        // Reset 290 min out of a 300 min period → only ~10 min (3.3%) elapsed.
        let resets_at = reset_in(290);
        assert_eq!(compute_predicted_pct(2.0, &resets_at, 300), None);
    }

    #[test]
    fn predicted_pct_linear_extrapolation_at_midpoint() {
        // 50% of the period elapsed, 30% used → linear projection ~60%.
        let resets_at = reset_in(150);
        let pred = compute_predicted_pct(30.0, &resets_at, 300).expect("should predict");
        assert!((pred - 60.0).abs() < 2.0, "expected ~60, got {pred}");
    }

    #[test]
    fn predicted_pct_parses_naive_timestamp_via_z_fallback() {
        // Claude.ai sometimes returns timestamps without an offset; the code
        // retries parsing with a trailing `Z`. Build a naive UTC string.
        let naive = (Utc::now() + Duration::minutes(150))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let pred = compute_predicted_pct(30.0, &Some(naive), 300).expect("should predict");
        assert!((pred - 60.0).abs() < 2.0, "expected ~60, got {pred}");
    }

    // ---- openai_payload ---------------------------------------------------

    /// The live shape from the user's account: 5-hour primary + 7-day secondary.
    const CHATGPT_BOTH: &str = r#"{
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {"used_percent": 1, "limit_window_seconds": 18000, "reset_at": 1788195954},
            "secondary_window": {"used_percent": 4, "limit_window_seconds": 604800, "reset_at": 1788762021}
        }
    }"#;

    fn chatgpt_usage(body: &str) -> crate::openai_fetcher::OpenAiUsage {
        crate::openai_fetcher::parse_usage(body).unwrap().unwrap()
    }

    #[test]
    fn openai_windows_land_in_the_slots_that_match_their_length() {
        let payload = openai_payload("chatgpt", &chatgpt_usage(CHATGPT_BOTH));

        assert_eq!(payload.five_hour_pct, 1.0);
        assert_eq!(payload.seven_day_pct, 4.0);
        assert_eq!(payload.five_hour_window_seconds, Some(18000));
        assert_eq!(payload.seven_day_window_seconds, Some(604800));
        assert_eq!(payload.provider, Provider::OpenaiWeb);
        assert!(payload.resets_at.is_some());
        assert!(payload.seven_day_resets_at.is_some());
    }

    /// A weekly-only account must not be shown a 5-hour window it never reported.
    #[test]
    fn openai_payload_marks_a_missing_short_window() {
        let body = r#"{"rate_limit":{"primary_window":{"used_percent":77,"limit_window_seconds":604800,"reset_at":1787222964},"secondary_window":null}}"#;
        let payload = openai_payload("chatgpt", &chatgpt_usage(body));

        assert_eq!(payload.seven_day_pct, 77.0);
        assert_eq!(payload.five_hour_window_seconds, None);
        assert_eq!(payload.resets_at, None);
        assert!(!payload.has_short_window());
        assert_eq!(payload.predicted_pct, None);
    }

    /// Claude reports both windows on every poll, so its payloads always say so —
    /// including for the UI's "is this bar real data?" check.
    #[test]
    fn claude_payloads_always_have_a_short_window() {
        let payload = UsagePayload {
            instance: "default".into(),
            provider: Provider::ClaudeWeb,
            five_hour_pct: 0.0,
            seven_day_pct: 0.0,
            resets_at: None,
            seven_day_resets_at: None,
            predicted_pct: None,
            five_hour_window_seconds: None,
            seven_day_window_seconds: None,
        };
        assert!(payload.has_short_window());
    }

    /// Pace is projected against the window ChatGPT reported, not Claude's 300
    /// minutes: half of a 5-hour window elapsed with 30% used projects to ~60%.
    #[test]
    fn openai_prediction_uses_the_reported_window_length() {
        let reset_at = (Utc::now() + Duration::minutes(150)).timestamp();
        let body = format!(
            r#"{{"rate_limit":{{"primary_window":{{"used_percent":30,"limit_window_seconds":18000,"reset_at":{reset_at}}},"secondary_window":{{"used_percent":4,"limit_window_seconds":604800,"reset_at":{reset_at}}}}}}}"#
        );
        let predicted = openai_payload("chatgpt", &chatgpt_usage(&body))
            .predicted_pct
            .expect("should predict");
        assert!((predicted - 60.0).abs() < 2.0, "expected ~60, got {predicted}");
    }

    // ---- process_limit ----------------------------------------------------

    fn process_five_hour(
        sender: &MockNotificationSender,
        config: &Config,
        state: &mut MonitorState,
        limit: &UsageLimit,
    ) {
        let other = UsageLimit { utilization: 0.0, resets_at: reset_in(10080) };
        process_limit(
            sender,
            config,
            state,
            "default",
            Provider::ClaudeWeb,
            LimitType::FiveHour,
            limit,
            300,
            LimitType::SevenDay,
            &other,
            false,
            "UTC",
        )
        .unwrap();
    }

    #[test]
    fn threshold_crossing_notifies_once_and_does_not_refire() {
        // Regression test: jumping to 80% crosses the 50 and 75 thresholds.
        // We expect exactly ONE notification, and a second identical poll must
        // NOT produce another. (Previously only the highest threshold was
        // marked notified, so the lower band re-fired on the next poll.)
        let config = Config::default();
        let sender = MockNotificationSender::new();
        let mut state = MonitorState::default();
        // Reset 50 min out → ~83% of the period elapsed, so 80% used projects to
        // ~96% (no overage warning) and we isolate the threshold-dedup behaviour.
        let l = limit(80.0, 50);

        process_five_hour(&sender, &config, &mut state, &l);
        assert_eq!(sender.count(), 1, "should notify once on the crossing poll");

        process_five_hour(&sender, &config, &mut state, &l);
        assert_eq!(
            sender.count(),
            1,
            "an identical follow-up poll must not re-notify a lower band"
        );

        // Both crossed bands should be recorded as notified.
        assert!(state.is_threshold_notified(LimitType::FiveHour, 50));
        assert!(state.is_threshold_notified(LimitType::FiveHour, 75));
    }

    #[test]
    fn threshold_not_notified_below_lowest_band() {
        let config = Config::default();
        let sender = MockNotificationSender::new();
        let mut state = MonitorState::default();
        let l = limit(10.0, 200);

        process_five_hour(&sender, &config, &mut state, &l);
        assert_eq!(sender.count(), 0, "10% is below the 50 threshold");
    }

    #[test]
    fn threshold_notifications_respect_disabled_flag() {
        let mut config = Config::default();
        config.notifications.notify_threshold_crossings = false;
        let sender = MockNotificationSender::new();
        let mut state = MonitorState::default();

        process_five_hour(&sender, &config, &mut state, &limit(80.0, 50));
        assert_eq!(sender.count(), 0, "disabled crossings must not notify");
    }

    #[test]
    fn predicted_overage_warns_when_on_track_to_exceed() {
        // 60% used at the midpoint of the period → projected ~120%.
        // Expect a threshold notification (50 band) plus an overage warning.
        let config = Config::default();
        let sender = MockNotificationSender::new();
        let mut state = MonitorState::default();

        process_five_hour(&sender, &config, &mut state, &limit(60.0, 150));
        assert_eq!(sender.count(), 2, "threshold + overage expected");
        assert!(!state.should_warn_overage(LimitType::FiveHour), "overage cooldown set");
    }

    #[test]
    fn upcoming_reset_notifies_once_within_window() {
        let mut config = Config::default();
        config.notifications.minutes_before_five_hour_reset = Some(15);
        let sender = MockNotificationSender::new();
        let mut state = MonitorState::default();
        // Reset 10 min out (<=15) and 0% used → only the upcoming-reset notif fires.
        let l = limit(0.0, 10);

        process_five_hour(&sender, &config, &mut state, &l);
        assert_eq!(sender.count(), 1, "upcoming reset should notify");
        assert!(state.is_upcoming_reset_notified(LimitType::FiveHour));

        process_five_hour(&sender, &config, &mut state, &l);
        assert_eq!(sender.count(), 1, "upcoming reset must not re-notify");
    }

    #[test]
    fn capacity_warning_fires_when_capacity_left_near_reset() {
        let mut config = Config::default();
        // Warn if <=60 min remain with >=20% capacity unused.
        config.notifications.capacity_warning_five_hour = Some((60, 20));
        let sender = MockNotificationSender::new();
        let mut state = MonitorState::default();
        // 30 min to reset, 10% used → 90% capacity left, threshold not crossed.
        let l = limit(10.0, 30);

        process_five_hour(&sender, &config, &mut state, &l);
        assert_eq!(sender.count(), 1, "capacity warning should fire");
        assert!(state.is_capacity_warning_notified(LimitType::FiveHour));
    }
}
