//! Credential storage and bearer minting for ChatGPT accounts.
//!
//! The stored secret is a single cookie, `__Secure-next-auth.session-token`
//! (verified sufficient on its own — the rest of the jar adds nothing). It lasts
//! ~90 days. Exchanging it at `/api/auth/session` yields a bearer good for
//! ~10 days, which is what `/wham/usage` actually authenticates with.
//!
//! **The exchange must use HTTP/1.1.** Cloudflare 403s the HTTP/2 fingerprint on
//! `/api/auth/*` and returns an HTML error page — for unauthenticated requests
//! too, so it is not a credential problem and no amount of header spoofing fixes
//! it. Over HTTP/1.1 there is no h2 fingerprint to match and the request passes.
//! `.http1_only()` below is load-bearing, not a preference.

use crate::crypto;
use crate::usage_fetcher::AuthRequiredError;
use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SESSION_URL: &str = "https://chatgpt.com/api/auth/session";
pub const SESSION_COOKIE: &str = "__Secure-next-auth.session-token";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// Re-mint this many seconds before the bearer actually expires, so a poll never
/// races the expiry boundary.
const REFRESH_MARGIN_SECS: i64 = 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiSession {
    /// The `__Secure-next-auth.session-token` cookie value.
    pub session_token: String,
    /// Cached bearer, re-minted from the cookie when it nears expiry.
    #[serde(default)]
    pub bearer: Option<String>,
    /// Unix seconds; `None` forces a re-mint.
    #[serde(default)]
    pub bearer_expires_at: Option<i64>,
}

impl OpenAiSession {
    pub fn new(session_token: String) -> Self {
        Self { session_token, bearer: None, bearer_expires_at: None }
    }

    pub fn load(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            anyhow::bail!("ChatGPT session file not found — add the account again to sign in.");
        }
        let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let envelope: crypto::Envelope = serde_json::from_value(value)?;
        let key = crypto::get_or_create_key(fallback_dir(path))?;
        Ok(serde_json::from_slice(&crypto::decrypt(&envelope, &key)?)?)
    }

    pub fn save(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let key = crypto::get_or_create_key(fallback_dir(path))?;
        let envelope = crypto::encrypt(&serde_json::to_vec(self)?, &key)?;
        std::fs::write(path, serde_json::to_string_pretty(&envelope)?)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// True when the cached bearer is still comfortably valid.
    fn bearer_is_fresh(&self, now: i64) -> bool {
        match (self.bearer.as_ref(), self.bearer_expires_at) {
            (Some(b), Some(exp)) => !b.is_empty() && exp - REFRESH_MARGIN_SECS > now,
            _ => false,
        }
    }

    /// Return a usable bearer, minting a new one if the cached one is stale.
    ///
    /// Sets `dirty` when a new token was minted, so the caller knows to persist.
    pub async fn ensure_bearer(&mut self, now: i64) -> Result<(String, bool)> {
        if self.bearer_is_fresh(now) {
            return Ok((self.bearer.clone().unwrap(), false));
        }
        let token = mint_bearer(&self.session_token).await?;
        self.bearer_expires_at = jwt_expiry(&token);
        self.bearer = Some(token.clone());
        Ok((token, true))
    }
}

/// Same resolution as `storage::fallback_dir`, so both providers share one
/// key-file location when no keychain backend is available.
fn fallback_dir(session_path: &Path) -> &Path {
    session_path.parent().unwrap_or_else(|| Path::new("."))
}

/// Read the `exp` claim without verifying the signature — we only need to know
/// when to re-mint, and the token came straight from OpenAI over TLS.
fn jwt_expiry(jwt: &str) -> Option<i64> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("exp")?.as_i64()
}

/// Exchange the session cookie for a bearer token.
pub async fn mint_bearer(session_token: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        // See the module docs: HTTP/2 is blocked by Cloudflare on this path.
        .http1_only()
        .build()?;

    let response = client
        .get(SESSION_URL)
        .header("Cookie", format!("{}={}", SESSION_COOKIE, session_token))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response.text().await?;

    // An HTML body here means the Cloudflare rule fired — almost certainly
    // because the request went out over HTTP/2. Say so, rather than reporting it
    // as an expired login and sending the user through a pointless re-auth.
    if content_type.contains("text/html") {
        anyhow::bail!(
            "chatgpt.com returned HTML (HTTP {}) instead of JSON — the request was blocked \
             before authentication. This means the HTTP/1.1-only client was bypassed.",
            status
        );
    }

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(anyhow::Error::new(AuthRequiredError {
            message: format!("HTTP {} — ChatGPT session cookie is no longer valid", status),
        }));
    }

    parse_access_token(&body)
}

/// Pull `accessToken` out of an `/api/auth/session` body.
///
/// A logged-out session returns valid JSON with no `accessToken`, which means an
/// expired cookie rather than a malformed response.
pub fn parse_access_token(body: &str) -> Result<String> {
    let parsed: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| anyhow::anyhow!("failed to parse session response: {}", e))?;

    match parsed.get("accessToken").and_then(|t| t.as_str()) {
        Some(token) if !token.is_empty() => Ok(token.to_string()),
        _ => Err(anyhow::Error::new(AuthRequiredError {
            message: "ChatGPT session returned no access token — the login has expired".to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `{"alg":"none"}` . `{"exp":2000000000}` . ""
    const JWT_EXP_2033: &str = "eyJhbGciOiJub25lIn0.eyJleHAiOjIwMDAwMDAwMDB9.";

    #[test]
    fn extracts_access_token() {
        let body = r#"{"user":{"id":"x"},"accessToken":"abc123","expires":"2026-11-25T07:22:23.710Z"}"#;
        assert_eq!(parse_access_token(body).unwrap(), "abc123");
    }

    /// The logged-out shape observed live: valid JSON, banner only, no token.
    #[test]
    fn logged_out_body_is_an_auth_error_not_a_parse_error() {
        let body = r#"{"WARNING_BANNER":"DO NOT SHARE ANY PART OF THE INFORMATION YOU SEE HERE."}"#;
        let err = parse_access_token(body).unwrap_err();
        assert!(err.downcast_ref::<AuthRequiredError>().is_some(), "got: {err}");
    }

    #[test]
    fn empty_access_token_is_an_auth_error() {
        let err = parse_access_token(r#"{"accessToken":""}"#).unwrap_err();
        assert!(err.downcast_ref::<AuthRequiredError>().is_some());
    }

    #[test]
    fn malformed_body_is_a_plain_error() {
        let err = parse_access_token("<html>nope</html>").unwrap_err();
        assert!(err.downcast_ref::<AuthRequiredError>().is_none());
    }

    #[test]
    fn reads_jwt_expiry() {
        assert_eq!(jwt_expiry(JWT_EXP_2033), Some(2_000_000_000));
    }

    #[test]
    fn unparseable_jwt_has_no_expiry() {
        assert_eq!(jwt_expiry("not.a.jwt"), None);
        assert_eq!(jwt_expiry("single-segment"), None);
    }

    #[test]
    fn fresh_bearer_is_reused() {
        let session = OpenAiSession {
            session_token: "cookie".into(),
            bearer: Some("tok".into()),
            bearer_expires_at: Some(10_000),
        };
        assert!(session.bearer_is_fresh(1_000));
    }

    /// Within the refresh margin the token is treated as stale, so a poll never
    /// races the expiry boundary.
    #[test]
    fn bearer_inside_refresh_margin_is_stale() {
        let session = OpenAiSession {
            session_token: "cookie".into(),
            bearer: Some("tok".into()),
            bearer_expires_at: Some(10_000),
        };
        assert!(!session.bearer_is_fresh(10_000 - REFRESH_MARGIN_SECS + 1));
    }

    #[test]
    fn missing_or_empty_bearer_is_stale() {
        let mut session = OpenAiSession::new("cookie".into());
        assert!(!session.bearer_is_fresh(0));
        session.bearer = Some(String::new());
        session.bearer_expires_at = Some(i64::MAX);
        assert!(!session.bearer_is_fresh(0));
    }

    /// Guards the one assumption this module cannot verify offline: that
    /// reqwest's HTTP/1.1 client is not caught by the Cloudflare rule that 403s
    /// HTTP/2 on `/api/auth/*`. Unauthenticated is enough to tell the two apart —
    /// a pass returns JSON, a block returns `text/html`.
    ///
    /// Network-dependent, so ignored by default:
    /// `cargo test -p claudar-core http1_only -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "hits chatgpt.com"]
    async fn http1_only_client_is_not_cloudflare_blocked() {
        let client = reqwest::Client::builder().http1_only().build().unwrap();
        let response = client
            .get(SESSION_URL)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .expect("request failed");

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let status = response.status();
        println!("HTTP {status} content-type={content_type}");

        assert!(
            !content_type.contains("text/html"),
            "Cloudflare blocked the HTTP/1.1 client (HTTP {status}) — the bypass no longer holds"
        );
        assert_eq!(status, reqwest::StatusCode::OK);
    }

    #[test]
    fn round_trips_through_encrypted_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions").join("chatgpt.json");
        let session = OpenAiSession {
            session_token: "secret-cookie".into(),
            bearer: Some("tok".into()),
            bearer_expires_at: Some(123),
        };
        session.save(&path).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("secret-cookie"), "cookie must not be stored in plaintext");

        let loaded = OpenAiSession::load(&path).unwrap();
        assert_eq!(loaded.session_token, "secret-cookie");
        assert_eq!(loaded.bearer_expires_at, Some(123));
    }
}
