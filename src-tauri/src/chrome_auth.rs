use anyhow::{anyhow, Result};
use claudar_core::openai_session::OpenAiSession;
use claudar_core::storage::SessionData;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio_tungstenite::tungstenite::Message;
use futures::{SinkExt, StreamExt};

/// Well-known install locations for a Chromium-based browser, in preference order.
///
/// Any of these speak the same CDP protocol the login flow drives, so Edge / Brave /
/// Chromium are accepted alongside Chrome. We check both machine-wide and per-user
/// locations because Chrome can be installed without admin rights (e.g. macOS
/// `~/Applications`, Windows `%LOCALAPPDATA%`).
fn browser_candidates() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        let apps = [
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "Brave Browser.app/Contents/MacOS/Brave Browser",
            "Chromium.app/Contents/MacOS/Chromium",
        ];
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        roots
            .iter()
            .flat_map(|root| apps.iter().map(move |a| root.join(a)))
            .collect()
    } else if cfg!(target_os = "windows") {
        let suffixes = [
            "Google\\Chrome\\Application\\chrome.exe",
            "Microsoft\\Edge\\Application\\msedge.exe",
            "BraveSoftware\\Brave-Browser\\Application\\brave.exe",
            "Chromium\\Application\\chrome.exe",
        ];
        let mut paths = Vec::new();
        for env_var in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
            if let Ok(base) = std::env::var(env_var) {
                for s in &suffixes {
                    paths.push(PathBuf::from(&base).join(s));
                }
            }
        }
        paths
    } else {
        // Linux and other unixes.
        [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/microsoft-edge",
            "/usr/bin/brave-browser",
            "/usr/bin/chromium-browser",
            "/usr/bin/chromium",
            "/snap/bin/chromium",
            "/snap/bin/google-chrome",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect()
    }
}

/// Executable names to look for on `$PATH` as a last resort when no well-known
/// install location matched (covers custom installs, distro packaging, Nix, etc.).
fn path_exe_names() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["chrome.exe", "msedge.exe", "brave.exe", "chromium.exe"]
    } else {
        &[
            "google-chrome",
            "google-chrome-stable",
            "microsoft-edge",
            "brave-browser",
            "chromium",
            "chromium-browser",
            "brave",
        ]
    }
}

/// Minimal `which`: return the first `$PATH` entry containing one of `names`.
fn search_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_chrome() -> Result<PathBuf> {
    for path in browser_candidates() {
        if path.exists() {
            return Ok(path);
        }
    }

    if let Some(path) = search_path(path_exe_names()) {
        return Ok(path);
    }

    Err(anyhow!(
        "No Chromium-based browser found. Claudar needs Google Chrome, \
         Microsoft Edge, Brave, or Chromium installed to sign in. \
         Please install one and try again."
    ))
}

pub(crate) fn find_free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Poll /json/list to get the WebSocket URL for the first page target.
pub(crate) async fn get_page_ws_url(port: u16) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/json/list", port);

    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        match client
            .get(&url)
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(json) = resp.json::<Value>().await {
                    if let Some(arr) = json.as_array() {
                        for target in arr {
                            let t = target.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if t == "page" {
                                if let Some(ws) = target
                                    .get("webSocketDebuggerUrl")
                                    .and_then(|u| u.as_str())
                                {
                                    return Ok(ws.to_string());
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }

    Err(anyhow!("Chrome page did not load within 30 seconds."))
}

/// What a login flow is looking for: where to send the browser, which cookie
/// domains are ours, and which cookie's arrival means "logged in".
struct AuthTarget {
    login_url: &'static str,
    domains: &'static [&'static str],
    sentinel: &'static str,
}

const CLAUDE_TARGET: AuthTarget = AuthTarget {
    login_url: "https://claude.ai/login",
    domains: &["claude.ai", "anthropic.com"],
    sentinel: "sessionKey",
};

const CHATGPT_TARGET: AuthTarget = AuthTarget {
    login_url: "https://chatgpt.com/auth/login",
    domains: &["chatgpt.com"],
    sentinel: claudar_core::openai_session::SESSION_COOKIE,
};

/// Pull every cookie belonging to `domains` out of a `Network.getAllCookies`
/// CDP response into `(name, value)` pairs.
fn extract_cookies(response: &Value, domains: &[&str]) -> Vec<(String, String)> {
    let mut cookies = Vec::new();
    if let Some(arr) = response.pointer("/result/cookies").and_then(|c| c.as_array()) {
        for c in arr {
            let domain = c.get("domain").and_then(|d| d.as_str()).unwrap_or("");
            if domains.iter().any(|d| domain.contains(d)) {
                let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                let value = c.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                cookies.push((name, value));
            }
        }
    }
    cookies
}

/// Drive a browser login and return the resulting cookie jar.
///
/// Detection = cookie polling. Every login path — Google OAuth popup, an emailed
/// code typed in-window, or a pasted magic link — ends with the target's sentinel
/// cookie existing. Poll `Network.getAllCookies` until it appears; the same
/// response carries the full jar we extract. This is flow-agnostic: it doesn't
/// matter which page/popup/redirect set it, which is why the same loop serves
/// both providers.
async fn capture_cookies(
    profile_path: PathBuf,
    debug_port: u16,
    target: &AuthTarget,
) -> Result<Vec<(String, String)>> {
    let chrome_bin = find_chrome()?;

    if profile_path.exists() {
        std::fs::remove_dir_all(&profile_path)?;
    }
    std::fs::create_dir_all(&profile_path)?;

    let profile_arg = profile_path.to_str().unwrap_or_default();

    // `--app` opens a clean, chromeless login window (no tabs/address bar) that
    // reads as part of Claudar; Google OAuth popups still open as their own
    // window. Instance separation comes from `--user-data-dir`.
    let mut child = tokio::process::Command::new(&chrome_bin)
        .arg(format!("--remote-debugging-port={}", debug_port))
        .arg(format!("--user-data-dir={}", profile_arg))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--window-size=480,720")
        .arg(format!("--app={}", target.login_url))
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn Chrome: {}", e))?;

    let ws_url = get_page_ws_url(debug_port).await?;

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| anyhow!("CDP connect failed: {}", e))?;

    let (mut ws_write, mut ws_read) = ws_stream.split();

    // `Network.getAllCookies` requires the Network domain enabled.
    ws_write
        .send(Message::Text(
            json!({"id": 1, "method": "Network.enable", "params": {}}).to_string(),
        ))
        .await?;

    let start = std::time::Instant::now();
    let max_wait = std::time::Duration::from_secs(600);
    let mut poll_id = 10u64;
    let mut poll_interval = tokio::time::interval(std::time::Duration::from_millis(500));
    let mut cookies: Vec<(String, String)> = Vec::new();

    'login: loop {
        if start.elapsed() > max_wait {
            let _ = child.kill().await;
            return Err(anyhow!("Login timeout after 10 minutes"));
        }

        tokio::select! {
            _ = poll_interval.tick() => {
                let msg = json!({"id": poll_id, "method": "Network.getAllCookies", "params": {}});
                if ws_write.send(Message::Text(msg.to_string())).await.is_err() {
                    break 'login;
                }
                poll_id += 1;
            }
            ws_msg = ws_read.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            // Only inspect getAllCookies responses (id >= 10).
                            if v.get("id").and_then(|i| i.as_u64()).map_or(false, |id| id >= 10) {
                                let found = extract_cookies(&v, target.domains);
                                if found.iter().any(|(k, val)| k == target.sentinel && !val.is_empty()) {
                                    cookies = found;
                                    break 'login;
                                }
                            }
                        }
                    }
                    Some(Err(e)) => { tracing::warn!("auth: CDP error: {}", e); break 'login; }
                    None => break 'login,
                    _ => {}
                }
            }
        }
    }

    let _ = child.kill().await;

    if cookies.is_empty() {
        return Err(anyhow!("Login not completed"));
    }

    Ok(cookies)
}

/// Log into Claude.ai and return the session cookies.
pub async fn run_chrome_auth(profile_path: PathBuf, debug_port: u16) -> Result<SessionData> {
    let cookies = capture_cookies(profile_path, debug_port, &CLAUDE_TARGET).await?;

    let full_cookie_string = cookies.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("; ");

    let get = |name: &str| -> Option<String> {
        cookies.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    };

    let session_key = get("sessionKey").unwrap_or_default();
    if session_key.is_empty() {
        return Err(anyhow!("sessionKey cookie not found — login may not have completed"));
    }

    Ok(SessionData {
        org_id: String::new(),
        session_key,
        cf_clearance: get("cf_clearance"),
        last_active_org: get("lastActiveOrg"),
        anthropic_device_id: get("anthropic-device-id"),
        cf_bm: get("__cf_bm"),
        ssid: get("__ssid"),
        full_cookie_string: if full_cookie_string.is_empty() { None } else { Some(full_cookie_string) },
    })
}

/// Log into ChatGPT and return an [`OpenAiSession`] holding the one cookie that
/// matters. The rest of the jar is discarded: `__Secure-next-auth.session-token`
/// alone is enough to mint bearers, and it lasts ~90 days.
pub async fn run_chatgpt_auth(profile_path: PathBuf, debug_port: u16) -> Result<OpenAiSession> {
    let cookies = capture_cookies(profile_path, debug_port, &CHATGPT_TARGET).await?;

    let session_token = cookies
        .iter()
        .find(|(k, _)| k == CHATGPT_TARGET.sentinel)
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow!("ChatGPT session cookie not found — login may not have completed"))?;

    Ok(OpenAiSession::new(session_token))
}
