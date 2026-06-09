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

/// Fetch the user's organization UUID from the Claude.ai API using session cookies.
///
/// Used right after an in-app login (wizard Step 2) to populate `SessionData.org_id`
/// without requiring the user to find it manually.
///
/// Account selection matters: `/api/organizations` can return several orgs (e.g. a
/// personal org plus a Team/Work org, or API-only orgs), and the `/usage` endpoint
/// returns **403** for any org that isn't the one the chat frontend is scoped to.
/// We mirror what the browser does: prefer the org named by the `lastActiveOrg`
/// cookie, then fall back to the first org that advertises a chat capability, then
/// to the first org of any kind.
pub async fn fetch_org_id(cookie_header: &str, preferred_org: Option<&str>) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client
        .get("https://claude.ai/api/organizations")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Referer", "https://claude.ai/")
        .header("Origin", "https://claude.ai")
        .header("Cookie", cookie_header)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(anyhow::Error::new(AuthRequiredError {
            message: format!("HTTP {} — could not fetch organizations", status),
        }));
    }

    let orgs: Vec<serde_json::Value> = serde_json::from_str(&body)
        .map_err(|e| anyhow::anyhow!("failed to parse organizations response: {} | body: {}", e, &body[..body.len().min(300)]))?;

    select_org_id(&orgs, preferred_org)
        .ok_or_else(|| anyhow::anyhow!("no organizations found for this account"))
}

/// Choose which organization to monitor from the `/api/organizations` response.
///
/// Preference order: the `preferred_org` UUID (the `lastActiveOrg` cookie the chat
/// frontend uses) → the first org advertising a `chat` capability (skips API-only
/// orgs that 403 on `/usage`) → the first org of any kind.
fn select_org_id(orgs: &[serde_json::Value], preferred_org: Option<&str>) -> Option<String> {
    let uuid_of = |org: &serde_json::Value| -> Option<String> {
        org.get("uuid").and_then(|u| u.as_str()).map(str::to_string)
    };

    if let Some(pref) = preferred_org.filter(|p| !p.is_empty()) {
        if let Some(org_id) = orgs.iter().filter_map(uuid_of).find(|u| u == pref) {
            return Some(org_id);
        }
    }

    let has_chat_capability = |org: &serde_json::Value| -> bool {
        org.get("capabilities")
            .and_then(|c| c.as_array())
            .map(|caps| caps.iter().any(|c| c.as_str() == Some("chat")))
            .unwrap_or(false)
    };
    if let Some(org_id) = orgs.iter().find(|o| has_chat_capability(o)).and_then(uuid_of) {
        return Some(org_id);
    }

    orgs.first().and_then(uuid_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefers_last_active_org_cookie() {
        let orgs = vec![
            json!({"uuid": "api-org", "capabilities": ["api"]}),
            json!({"uuid": "chat-org", "capabilities": ["chat"]}),
        ];
        assert_eq!(
            select_org_id(&orgs, Some("api-org")).as_deref(),
            Some("api-org")
        );
    }

    #[test]
    fn skips_api_only_org_when_no_preference() {
        let orgs = vec![
            json!({"uuid": "api-org", "capabilities": ["api"]}),
            json!({"uuid": "chat-org", "capabilities": ["chat", "claude_pro"]}),
        ];
        assert_eq!(
            select_org_id(&orgs, None).as_deref(),
            Some("chat-org")
        );
    }

    #[test]
    fn ignores_empty_or_unmatched_preference() {
        let orgs = vec![
            json!({"uuid": "api-org", "capabilities": ["api"]}),
            json!({"uuid": "chat-org", "capabilities": ["chat"]}),
        ];
        assert_eq!(select_org_id(&orgs, Some("")).as_deref(), Some("chat-org"));
        assert_eq!(select_org_id(&orgs, Some("nope")).as_deref(), Some("chat-org"));
    }

    #[test]
    fn falls_back_to_first_org_without_capabilities() {
        let orgs = vec![
            json!({"uuid": "first"}),
            json!({"uuid": "second"}),
        ];
        assert_eq!(select_org_id(&orgs, None).as_deref(), Some("first"));
    }

    #[test]
    fn none_when_empty() {
        assert_eq!(select_org_id(&[], None), None);
    }
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
