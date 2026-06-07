use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

/// Returned when session cookies are expired, revoked, or blocked by Cloudflare.
/// The monitor loop catches this variant and logs a warning instead of crashing.
#[derive(Debug)]
pub struct AuthRequiredError {
    pub message: String,
}

impl std::fmt::Display for AuthRequiredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "auth required: {}", self.message)
    }
}

impl std::error::Error for AuthRequiredError {}

#[derive(Debug, Deserialize)]
pub struct UsageResponse {
    pub five_hour: UsageLimit,
    pub seven_day: UsageLimit,
}

#[derive(Debug, Deserialize)]
pub struct UsageLimit {
    pub utilization: f64,
    #[serde(default)]
    pub resets_at: Option<String>,
}

/// Fetch usage data from the Claude.ai API using stored session cookies.
///
/// On HTTP 401/403 or a Cloudflare challenge page the error downcasts to
/// [`AuthRequiredError`].  The caller (monitor loop) checks for this variant
/// and emits a warning rather than propagating the error or crashing.
pub async fn fetch_usage(cookie_header: &str, org_id: &str) -> Result<serde_json::Value> {
    let url = format!("https://claude.ai/api/organizations/{}/usage", org_id);
    tracing::info!("usage_fetch: attempting for org {}", org_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Referer", "https://claude.ai/settings/usage")
        .header("Origin", "https://claude.ai")
        .header(
            "sec-ch-ua",
            r#""Google Chrome";v="131", "Chromium";v="131", "Not_A Brand";v="24""#,
        )
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", r#""macOS""#)
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .header("Cookie", cookie_header)
        .send()
        .await?;

    let status = response.status();
    tracing::info!("usage_fetch: response status {}", status);

    let body = response.text().await?;

    // Cloudflare challenge pages contain these markers
    if body.contains("Just a moment")
        || body.contains("cf-browser-verification")
        || body.contains("_cf_chl")
    {
        tracing::warn!(
            "usage_fetch: Cloudflare challenge detected for org {}",
            org_id
        );
        return Err(anyhow::Error::new(AuthRequiredError {
            message: "Cloudflare challenge — re-authentication required".to_string(),
        }));
    }

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let preview = &body[..body.len().min(300)];
        tracing::warn!(
            "usage_fetch: auth failure ({}) for org {}: {}",
            status,
            org_id,
            preview
        );
        return Err(anyhow::Error::new(AuthRequiredError {
            message: format!("HTTP {} — session expired or invalid", status),
        }));
    }

    if !status.is_success() {
        let preview = &body[..body.len().min(300)];
        tracing::warn!(
            "usage_fetch: non-success status {} for org {}: {}",
            status,
            org_id,
            preview
        );
        anyhow::bail!("HTTP {}: {}", status, preview);
    }

    // HTML response indicates an auth redirect or Cloudflare JS challenge
    if body.trim_start().starts_with('<') {
        tracing::warn!(
            "usage_fetch: received HTML instead of JSON for org {} — session may have expired",
            org_id
        );
        return Err(anyhow::Error::new(AuthRequiredError {
            message: "HTML response received — session cookies may have expired".to_string(),
        }));
    }

    tracing::debug!(
        "usage_fetch: raw response: {}",
        &body[..body.len().min(500)]
    );

    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        tracing::warn!(
            "usage_fetch: failed to parse JSON: {} | body: {}",
            e,
            &body[..body.len().min(200)]
        );
        anyhow::anyhow!("Failed to parse JSON response: {}", e)
    })?;

    // Warn if the response looks like an unauthenticated/empty result
    if let (Some(fh), Some(sd)) = (json.get("five_hour"), json.get("seven_day")) {
        let fh_util = fh.get("utilization").and_then(|v| v.as_f64()).unwrap_or(-1.0);
        let sd_util = sd.get("utilization").and_then(|v| v.as_f64()).unwrap_or(-1.0);
        let fh_resets = fh.get("resets_at").and_then(|v| v.as_str());
        if fh_util == 0.0 && sd_util == 0.0 && fh_resets.is_none() {
            tracing::warn!(
                "usage_fetch: zero usage + null reset times for org {} — cookies may be expired",
                org_id
            );
        }
    }

    tracing::info!("usage_fetch: succeeded for org {}", org_id);
    Ok(json)
}
