//! Usage fetching for ChatGPT accounts.
//!
//! `GET https://chatgpt.com/backend-api/wham/usage` reports rolling-window usage
//! the same way Claude's `/usage` does: a percentage plus a reset timestamp.
//! A bearer token is the only required header — verified against a live account;
//! `chatgpt-account-id` merely populates `account_id` in the response, and none
//! of the browser's `oai-client-*` headers are needed.
//!
//! The number and order of windows is *not* fixed. Accounts have been observed
//! with `primary_window` as a 7-day window and no secondary, and with a 5-hour
//! primary plus a 7-day secondary. Nothing here may assume a position; see
//! [`UsageWindow::longest`].

use crate::usage_fetcher::AuthRequiredError;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Duration;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct WhamResponse {
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit: Option<RateLimit>,
}

#[derive(Debug, Deserialize)]
struct RateLimit {
    #[serde(default)]
    primary_window: Option<UsageWindow>,
    #[serde(default)]
    secondary_window: Option<UsageWindow>,
}

/// One rolling window. `used_percent` is 0-100; `reset_at` is a Unix timestamp
/// in seconds (the field is named `resets_at` in Codex's internal structs, but
/// the wire format uses `reset_at`).
#[derive(Debug, Clone, Deserialize)]
pub struct UsageWindow {
    pub used_percent: f64,
    pub limit_window_seconds: i64,
    #[serde(default)]
    pub reset_at: Option<i64>,
}

impl UsageWindow {
    /// The longest of the available windows — the "weekly" one in practice.
    ///
    /// Chosen by duration rather than by field name because the same account can
    /// report its 7-day window as either `primary_window` or `secondary_window`
    /// depending on the plan.
    fn longest(rate_limit: &RateLimit) -> Option<&UsageWindow> {
        [rate_limit.primary_window.as_ref(), rate_limit.secondary_window.as_ref()]
            .into_iter()
            .flatten()
            .max_by_key(|w| w.limit_window_seconds)
    }

    /// Reset time as an ISO 8601 string, matching the shape Claude's API returns
    /// so the UI can treat both providers identically.
    pub fn resets_at_iso(&self) -> Option<String> {
        DateTime::<Utc>::from_timestamp(self.reset_at?, 0).map(|dt| dt.to_rfc3339())
    }
}

/// A ChatGPT account's usage windows.
///
/// `weekly` is the longest window — the one the dashboard draws. `short` is the
/// other window when the account has two (typically 5-hour). It is carried even
/// though nothing renders it yet, because `poll_instance` persists every payload
/// to history; a placeholder would write a value that was never true.
#[derive(Debug, Clone)]
pub struct OpenAiUsage {
    pub pct: f64,
    pub resets_at: Option<String>,
    /// Duration of the window in seconds, so the UI can label it honestly
    /// instead of hardcoding "weekly".
    pub window_seconds: i64,
    pub short_pct: Option<f64>,
    pub short_resets_at: Option<String>,
    pub plan_type: Option<String>,
}

/// Parse a `/wham/usage` body into the single window worth showing.
///
/// Returns `Ok(None)` when the account reports no windows at all — a legitimate
/// state (seen as `secondary_window: null` alongside a null primary), not an error.
pub fn parse_usage(body: &str) -> Result<Option<OpenAiUsage>> {
    let parsed: WhamResponse = serde_json::from_str(body).map_err(|e| {
        anyhow::anyhow!("failed to parse wham usage response: {} | body: {}", e, &body[..body.len().min(300)])
    })?;

    let Some(rate_limit) = parsed.rate_limit.as_ref() else {
        return Ok(None);
    };
    let Some(window) = UsageWindow::longest(rate_limit) else {
        return Ok(None);
    };

    // The remaining window, if this account reports two. Compared by duration
    // because either field can hold the longer one.
    let short = [rate_limit.primary_window.as_ref(), rate_limit.secondary_window.as_ref()]
        .into_iter()
        .flatten()
        .find(|w| w.limit_window_seconds < window.limit_window_seconds);

    Ok(Some(OpenAiUsage {
        pct: window.used_percent,
        resets_at: window.resets_at_iso(),
        window_seconds: window.limit_window_seconds,
        short_pct: short.map(|w| w.used_percent),
        short_resets_at: short.and_then(|w| w.resets_at_iso()),
        plan_type: parsed.plan_type.clone(),
    }))
}

/// Fetch usage for a ChatGPT account using a bearer token.
///
/// Returns [`AuthRequiredError`] on 401/403 so the caller can reuse the existing
/// re-authentication path rather than treating an expired token as a crash.
pub async fn fetch_usage(bearer: &str) -> Result<Option<OpenAiUsage>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {}", bearer))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(anyhow::Error::new(AuthRequiredError {
            message: format!("HTTP {} from wham/usage — ChatGPT session needs re-authentication", status),
        }));
    }
    if !status.is_success() {
        anyhow::bail!("wham/usage returned HTTP {}", status);
    }

    parse_usage(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape captured live: 5-hour primary + 7-day secondary.
    const BOTH_WINDOWS: &str = r#"{
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {"used_percent": 1, "limit_window_seconds": 18000, "reset_at": 1787782677},
            "secondary_window": {"used_percent": 11, "limit_window_seconds": 604800, "reset_at": 1788294153}
        }
    }"#;

    /// Shape recorded in docs/04-multi-provider.md §3.1: 7-day *primary*, no secondary.
    const PRIMARY_ONLY: &str = r#"{
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {"used_percent": 77, "limit_window_seconds": 604800, "reset_at": 1787222964},
            "secondary_window": null
        }
    }"#;

    #[test]
    fn picks_the_seven_day_window_when_it_is_secondary() {
        let usage = parse_usage(BOTH_WINDOWS).unwrap().unwrap();
        assert_eq!(usage.pct, 11.0);
        assert_eq!(usage.window_seconds, 604800);
    }

    #[test]
    fn carries_the_shorter_window_when_present() {
        let usage = parse_usage(BOTH_WINDOWS).unwrap().unwrap();
        assert_eq!(usage.short_pct, Some(1.0));
        assert!(usage.short_resets_at.is_some());
    }

    /// One window means no shorter one — not a duplicate of the weekly value.
    #[test]
    fn single_window_has_no_short_counterpart() {
        let usage = parse_usage(PRIMARY_ONLY).unwrap().unwrap();
        assert_eq!(usage.pct, 77.0);
        assert_eq!(usage.short_pct, None);
    }

    /// The reason `longest` exists: the weekly window is not always `secondary`.
    #[test]
    fn picks_the_seven_day_window_when_it_is_primary() {
        let usage = parse_usage(PRIMARY_ONLY).unwrap().unwrap();
        assert_eq!(usage.pct, 77.0);
        assert_eq!(usage.window_seconds, 604800);
    }

    #[test]
    fn converts_reset_at_to_iso() {
        let usage = parse_usage(BOTH_WINDOWS).unwrap().unwrap();
        assert!(usage.resets_at.unwrap().starts_with("2026-"));
    }

    #[test]
    fn missing_reset_at_is_not_an_error() {
        let body = r#"{"rate_limit":{"primary_window":{"used_percent":5,"limit_window_seconds":604800}}}"#;
        let usage = parse_usage(body).unwrap().unwrap();
        assert_eq!(usage.pct, 5.0);
        assert!(usage.resets_at.is_none());
    }

    #[test]
    fn no_windows_yields_none_rather_than_erroring() {
        let body = r#"{"plan_type":"free","rate_limit":{"primary_window":null,"secondary_window":null}}"#;
        assert!(parse_usage(body).unwrap().is_none());
    }

    #[test]
    fn absent_rate_limit_yields_none() {
        assert!(parse_usage(r#"{"plan_type":"free"}"#).unwrap().is_none());
    }

    /// Unknown fields keep appearing on this endpoint (`promo`, `rate_limit_reset_credits`,
    /// `approx_local_messages`); they must not break parsing.
    #[test]
    fn tolerates_unknown_fields() {
        let body = r#"{
            "plan_type":"plus","email":"x@example.com","promo":null,
            "rate_limit":{"primary_window":{"used_percent":3,"limit_window_seconds":604800,"reset_at":1788294153},"secondary_window":null},
            "credits":{"approx_local_messages":[0,0]},"rate_limit_reset_credits":{"available_count":1}
        }"#;
        assert_eq!(parse_usage(body).unwrap().unwrap().pct, 3.0);
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_usage("not json").is_err());
    }
}
