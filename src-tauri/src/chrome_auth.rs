use anyhow::{anyhow, Result};
use claudar_core::storage::SessionData;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio_tungstenite::tungstenite::Message;
use futures::{SinkExt, StreamExt};

fn find_chrome() -> Result<PathBuf> {
    let candidates: Vec<PathBuf> = if cfg!(target_os = "macos") {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect()
    } else if cfg!(target_os = "linux") {
        vec![
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
            "/usr/bin/chromium",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect()
    } else if cfg!(target_os = "windows") {
        let mut paths = Vec::new();
        for env_var in ["PROGRAMFILES", "PROGRAMFILES(X86)"] {
            if let Ok(pf) = std::env::var(env_var) {
                paths.push(PathBuf::from(&pf).join("Google\\Chrome\\Application\\chrome.exe"));
                paths.push(PathBuf::from(&pf).join("Microsoft\\Edge\\Application\\msedge.exe"));
            }
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            paths.push(PathBuf::from(&local).join("Google\\Chrome\\Application\\chrome.exe"));
            paths.push(PathBuf::from(&local).join("Chromium\\Application\\chrome.exe"));
        }
        paths
    } else {
        vec![]
    };

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(anyhow!(
        "Chrome or Chromium not found. Please install Google Chrome."
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

/// Pull every claude.ai/anthropic.com cookie out of a `Network.getAllCookies`
/// CDP response into `(name, value)` pairs.
fn extract_cookies(response: &Value) -> Vec<(String, String)> {
    let mut cookies = Vec::new();
    if let Some(arr) = response.pointer("/result/cookies").and_then(|c| c.as_array()) {
        for c in arr {
            let domain = c.get("domain").and_then(|d| d.as_str()).unwrap_or("");
            if domain.contains("claude.ai") || domain.contains("anthropic.com") {
                let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                let value = c.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                cookies.push((name, value));
            }
        }
    }
    cookies
}

pub async fn run_chrome_auth(profile_path: PathBuf, debug_port: u16) -> Result<SessionData> {
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
        .arg("--app=https://claude.ai/login")
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

    // Detection = cookie polling. Every login path — Google OAuth popup, an
    // emailed code typed in-window, or a pasted magic link — ends with the
    // `sessionKey` cookie existing for claude.ai. Poll `Network.getAllCookies`
    // until it appears; the same response carries the full jar we extract. This
    // is flow-agnostic: it doesn't matter which page/popup/redirect set it.
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
                                let found = extract_cookies(&v);
                                if found.iter().any(|(k, val)| k == "sessionKey" && !val.is_empty()) {
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
