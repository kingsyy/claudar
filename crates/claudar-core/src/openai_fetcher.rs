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
//! primary plus a 7-day secondary. Nothing here may assume a position; the
//! windows are sorted by duration in [`parse_usage`].

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
    primary_window: Option<RawWindow>,
    #[serde(default)]
    secondary_window: Option<RawWindow>,
}

/// One rolling window as it comes off the wire. `used_percent` is 0-100;
/// `reset_at` is a Unix timestamp in seconds (the field is named `resets_at` in
/// Codex's internal structs, but the wire format uses `reset_at`).
#[derive(Debug, Clone, Deserialize)]
struct RawWindow {
    used_percent: f64,
    limit_window_seconds: i64,
    #[serde(default)]
    reset_at: Option<i64>,
}

/// One rolling window in the shape the rest of Claudar speaks: a percentage and
/// an ISO 8601 reset string, matching what Claude's API returns so both
/// providers travel the same code path.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiWindow {
    pub pct: f64,
    pub resets_at: Option<String>,
    /// Duration of the window in seconds, so callers can compute pace against
    /// the real period instead of assuming Claude's 5-hour / 7-day pair.
    pub window_seconds: i64,
}

impl RawWindow {
    fn into_window(self) -> OpenAiWindow {
        OpenAiWindow {
            pct: self.used_percent,
            resets_at: self
                .reset_at
                .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
                .map(|dt| dt.to_rfc3339()),
            window_seconds: self.limit_window_seconds,
        }
    }
}

/// A ChatGPT account's usage windows, ordered by duration rather than by the
/// field they arrived in — the same account can report its 7-day window as
/// either `primary_window` or `secondary_window` depending on the plan.
#[derive(Debug, Clone)]
pub struct OpenAiUsage {
    /// The longest window — 7 days on every account seen so far.
    pub long: OpenAiWindow,
    /// The shorter window (5 hours in practice) when the account reports two.
    /// `None` is a real state, not a missing value: some accounts report only
    /// a weekly window, and a zeroed placeholder would be a value that was
    /// never true.
    pub short: Option<OpenAiWindow>,
    pub plan_type: Option<String>,
}

/// Parse a `/wham/usage` body into its usage windows.
///
/// Returns `Ok(None)` when the account reports no windows at all — a legitimate
/// state (seen as `secondary_window: null` alongside a null primary), not an error.
pub fn parse_usage(body: &str) -> Result<Option<OpenAiUsage>> {
    let parsed: WhamResponse = serde_json::from_str(body).map_err(|e| {
        anyhow::anyhow!("failed to parse wham usage response: {} | body: {}", e, &body[..body.len().min(300)])
    })?;

    let Some(rate_limit) = parsed.rate_limit else {
        return Ok(None);
    };

    let mut windows: Vec<OpenAiWindow> = [rate_limit.primary_window, rate_limit.secondary_window]
        .into_iter()
        .flatten()
        .map(RawWindow::into_window)
        .collect();
    windows.sort_by_key(|w| w.window_seconds);

    let Some(long) = windows.pop() else {
        return Ok(None);
    };

    Ok(Some(OpenAiUsage {
        long,
        short: windows.pop(),
        plan_type: parsed.plan_type,
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
        assert_eq!(usage.long.pct, 11.0);
        assert_eq!(usage.long.window_seconds, 604800);
    }

    #[test]
    fn carries_the_shorter_window_when_present() {
        let short = parse_usage(BOTH_WINDOWS).unwrap().unwrap().short.unwrap();
        assert_eq!(short.pct, 1.0);
        assert_eq!(short.window_seconds, 18000);
        assert!(short.resets_at.is_some());
    }

    /// One window means no shorter one — not a duplicate of the weekly value.
    #[test]
    fn single_window_has_no_short_counterpart() {
        let usage = parse_usage(PRIMARY_ONLY).unwrap().unwrap();
        assert_eq!(usage.long.pct, 77.0);
        assert_eq!(usage.short, None);
    }

    /// The reason the windows are sorted: the weekly one is not always `secondary`.
    #[test]
    fn picks_the_seven_day_window_when_it_is_primary() {
        let usage = parse_usage(PRIMARY_ONLY).unwrap().unwrap();
        assert_eq!(usage.long.pct, 77.0);
        assert_eq!(usage.long.window_seconds, 604800);
    }

    #[test]
    fn converts_reset_at_to_iso() {
        let usage = parse_usage(BOTH_WINDOWS).unwrap().unwrap();
        assert!(usage.long.resets_at.unwrap().starts_with("2026-"));
    }

    #[test]
    fn missing_reset_at_is_not_an_error() {
        let body = r#"{"rate_limit":{"primary_window":{"used_percent":5,"limit_window_seconds":604800}}}"#;
        let usage = parse_usage(body).unwrap().unwrap();
        assert_eq!(usage.long.pct, 5.0);
        assert!(usage.long.resets_at.is_none());
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
        assert_eq!(parse_usage(body).unwrap().unwrap().long.pct, 3.0);
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_usage("not json").is_err());
    }
}
