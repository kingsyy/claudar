use crate::monitor_loop;
use claude_notify_core::{
    config::{Config, InstanceConfig},
    history::{self, HistoryRecord},
    monitor::{poll_instance, UsagePayload},
    storage::SessionData,
    usage_fetcher::fetch_org_id,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InstanceInfo {
    pub name: String,
    /// Whether a session file exists for this instance.
    pub has_session: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHistoryArgs {
    pub instance: Option<String>,
    pub since_days: i64,
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Return all configured instances with their session status.
#[tauri::command]
pub fn get_instances() -> Result<Vec<InstanceInfo>, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    let infos = config
        .effective_instances()
        .into_iter()
        .map(|inst| {
            let session_path = config
                .session_path_for(&inst.name)
                .map(|p| p.exists())
                .unwrap_or(false);
            InstanceInfo {
                name: inst.name,
                has_session: session_path,
            }
        })
        .collect();
    Ok(infos)
}

/// Return the current config (the `instance` argument is reserved for future per-instance config).
#[tauri::command]
pub fn get_config(_instance: Option<String>) -> Result<Config, String> {
    Config::load().map_err(|e| e.to_string())
}

/// Persist a modified config to disk.
#[tauri::command]
pub fn set_config(_instance: Option<String>, config: Config) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())
}

/// On-demand usage fetch for a single instance (bypasses the background loop).
/// Fires desktop notifications and writes a history record just like the loop does.
#[tauri::command]
pub async fn get_usage(instance: Option<String>) -> Result<UsagePayload, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    let name = instance.as_deref().unwrap_or("default");
    poll_instance(&config, name)
        .await
        .map_err(|e| e.to_string())
}

/// Return history records for an instance, filtered to the last `since_days` days.
#[tauri::command]
pub fn get_history(args: GetHistoryArgs) -> Result<Vec<HistoryRecord>, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    let name = args.instance.as_deref().unwrap_or("default");
    let records = history::load_records(&config, name).map_err(|e| e.to_string())?;

    let cutoff = Utc::now() - ChronoDuration::days(args.since_days);
    Ok(records
        .into_iter()
        .filter(|r| r.polled_at >= cutoff)
        .collect())
}

// ─── Instance management ──────────────────────────────────────────────────────

/// Add a new named instance to the config and start its background poll task.
/// The caller is responsible for triggering `start_auth` for the new instance.
#[tauri::command]
pub fn add_instance(app: AppHandle, name: String) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Instance name cannot be empty".to_string());
    }

    let mut config = Config::load().map_err(|e| e.to_string())?;
    if config.effective_instances().iter().any(|i| i.name == name) {
        return Err(format!("Instance '{}' already exists", name));
    }

    // Switching from the implicit "default" instance to an explicit list — keep "default" too.
    if config.instances.is_empty() {
        config.instances.push(InstanceConfig {
            name: "default".to_string(),
        });
    }
    config.instances.push(InstanceConfig { name: name.clone() });
    config.save().map_err(|e| e.to_string())?;

    monitor_loop::spawn_instance_task(app, name);

    Ok(())
}

/// Stop an instance's monitor task and remove its config entry, session, state,
/// and history files.
#[tauri::command]
pub fn remove_instance(app: AppHandle, instance: String) -> Result<(), String> {
    let mut config = Config::load().map_err(|e| e.to_string())?;

    if config.effective_instances().len() <= 1 {
        return Err("Cannot remove the only configured account".to_string());
    }

    monitor_loop::stop_instance_task(&app, &instance);

    config.instances.retain(|i| i.name != instance);
    config.save().map_err(|e| e.to_string())?;

    for path in [
        config.session_path_for(&instance),
        config.state_path_for(&instance),
        config.history_path_for(&instance),
    ] {
        if let Ok(path) = path {
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    Ok(())
}

// ─── Onboarding / Auth ────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct AuthCompletePayload {
    instance: String,
}

#[derive(Clone, Serialize)]
struct AuthErrorPayload {
    instance: String,
    message: String,
}

/// Open Claude.ai login in the system browser (avoiding popup blocks).
/// Shows a helper window while the user authenticates.
#[tauri::command]
pub async fn start_auth(app: AppHandle, instance: Option<String>) -> Result<(), String> {
    let instance_name = instance.unwrap_or_else(|| "default".to_string());

    if let Some(existing) = app.get_webview_window("auth") {
        let _ = existing.close();
    }

    // Open Claude.ai login in the system browser
    let login_url = "https://claude.ai/login";
    open::that(login_url)
        .map_err(|e| format!("Failed to open browser. Please visit {} manually: {}", login_url, e))?;

    // Show a helper window explaining what to do
    let html = r#"<!DOCTYPE html>
<html>
<head>
  <title>Claude Notify — Log In</title>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      padding: 20px;
    }
    .container {
      background: white;
      border-radius: 12px;
      box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
      max-width: 460px;
      padding: 40px;
    }
    h1 {
      font-size: 28px;
      margin-bottom: 8px;
      color: #1a1a1a;
      font-weight: 600;
    }
    .subtitle {
      color: #666;
      font-size: 14px;
      margin-bottom: 24px;
    }
    .step {
      display: flex;
      gap: 16px;
      margin-bottom: 20px;
      padding: 16px;
      background: #f9f9f9;
      border-radius: 8px;
      border-left: 3px solid #667eea;
    }
    .step-number {
      min-width: 32px;
      width: 32px;
      height: 32px;
      background: #667eea;
      color: white;
      border-radius: 50%;
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 600;
      font-size: 14px;
    }
    .step-content {
      flex: 1;
    }
    .step-content p {
      color: #333;
      font-size: 14px;
      line-height: 1.5;
      margin: 0;
    }
    .step-content strong {
      color: #1a1a1a;
    }
    .buttons {
      display: flex;
      gap: 12px;
      margin-top: 24px;
    }
    button {
      flex: 1;
      padding: 12px 16px;
      border: none;
      border-radius: 6px;
      font-size: 14px;
      font-weight: 600;
      cursor: pointer;
      transition: all 0.2s;
    }
    .btn-primary {
      background: #667eea;
      color: white;
    }
    .btn-primary:hover {
      background: #5568d3;
    }
    .btn-secondary {
      background: #f0f0f0;
      color: #333;
    }
    .btn-secondary:hover {
      background: #e0e0e0;
    }
    button:disabled {
      opacity: 0.6;
      cursor: not-allowed;
    }
    .spinner {
      display: inline-block;
      width: 14px;
      height: 14px;
      border: 2px solid #f0f0f0;
      border-top: 2px solid #667eea;
      border-radius: 50%;
      animation: spin 0.8s linear infinite;
      margin-right: 8px;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
  </style>
</head>
<body>
  <div class="container">
    <h1>✓ Log In to Claude.ai</h1>
    <p class="subtitle">Your browser has opened to claude.ai/login</p>

    <div class="step">
      <div class="step-number">1</div>
      <div class="step-content">
        <p><strong>Sign in using Google or email</strong> in the browser window that just opened.</p>
      </div>
    </div>

    <div class="step">
      <div class="step-number">2</div>
      <div class="step-content">
        <p>Once you're logged in, click <strong>"Already logged in"</strong> below.</p>
      </div>
    </div>

    <div class="step">
      <div class="step-number">3</div>
      <div class="step-content">
        <p>Claude Notify will verify your session and continue setup.</p>
      </div>
    </div>

    <div class="buttons">
      <button class="btn-primary" onclick="handleCheck()">
        <span class="spinner"></span> Already logged in
      </button>
      <button class="btn-secondary" onclick="window.close()">Cancel</button>
    </div>
  </div>

  <script>
    let isChecking = false;

    async function handleCheck() {
      if (isChecking) return;
      isChecking = true;

      const btn = document.querySelector('.btn-primary');
      btn.disabled = true;

      try {
        const result = await window.__tauri__.invoke('check_auth_session', { instance: null });
        // Window will be closed by the backend on success
      } catch (error) {
        btn.disabled = false;
        isChecking = false;
        alert('Error: ' + error);
      }
    }

    // Auto-check every 3 seconds in case user logged in via browser
    setInterval(async () => {
      if (isChecking) return;
      try {
        await window.__tauri__.invoke('check_auth_session', { instance: null });
      } catch (e) {
        // Not logged in yet, continue waiting
      }
    }, 3000);
  </script>
</body>
</html>"#;

    let url = WebviewUrl::External(
        format!("data:text/html,{}", urlencoding::encode(html))
            .parse::<tauri::Url>()
            .map_err(|e| e.to_string())?,
    );

    let app_clone = app.clone();
    let instance_clone = instance_name.clone();

    let window = WebviewWindowBuilder::new(&app, "auth", url)
        .title("Claude Notify — Log In")
        .inner_size(500.0, 560.0)
        .build()
        .map_err(|e| e.to_string())?;

    let app_for_event = app_clone.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            // Silently close
        }
    });

    let _ = window.show();

    Ok(())
}

/// Check if the user is logged in by attempting to fetch their org ID from Claude.ai.
#[tauri::command]
pub async fn check_auth_session(app: AppHandle, instance: Option<String>) -> Result<(), String> {
    let instance_name = instance.unwrap_or_else(|| "default".to_string());

    // Try to fetch org ID using the session that might exist in the system browser
    // For now, this will fail because we can't access the system browser's cookies
    // The user needs to use the CLI: `claude-notify setup` for proper auth

    emit_auth_error(
        &app,
        &instance_name,
        "Please use the command-line setup instead:\n\n  claude-notify setup\n\nThis provides more reliable Google OAuth support.".to_string(),
    );

    Ok(())
}

/// Extract cookies from the auth webview, resolve the org id, persist the session,
/// close the webview, and emit `auth-complete`/`auth-error`.
async fn complete_auth(app: AppHandle, webview: tauri::WebviewWindow, instance: String) {
    let cookies = match webview.cookies() {
        Ok(c) => c,
        Err(e) => {
            emit_auth_error(&app, &instance, e.to_string());
            return;
        }
    };

    let mut session_key = None;
    let mut cf_clearance = None;
    let mut last_active_org = None;
    let mut anthropic_device_id = None;
    let mut cf_bm = None;
    let mut ssid = None;
    let mut pairs = Vec::new();

    for cookie in &cookies {
        let name = cookie.name();
        let value = cookie.value();
        pairs.push(format!("{}={}", name, value));
        match name {
            "sessionKey" => session_key = Some(value.to_string()),
            "cf_clearance" => cf_clearance = Some(value.to_string()),
            "lastActiveOrg" => last_active_org = Some(value.to_string()),
            "anthropic-device-id" => anthropic_device_id = Some(value.to_string()),
            "__cf_bm" => cf_bm = Some(value.to_string()),
            "__ssid" => ssid = Some(value.to_string()),
            _ => {}
        }
    }

    let full_cookie_string = pairs.join("; ");

    let session_key = match session_key {
        Some(key) => key,
        None => {
            emit_auth_error(
                &app,
                &instance,
                "No session cookie found — please try logging in again".to_string(),
            );
            return;
        }
    };

    let org_id = match fetch_org_id(&full_cookie_string).await {
        Ok(id) => id,
        Err(e) => {
            emit_auth_error(
                &app,
                &instance,
                format!("Logged in, but couldn't determine your organization: {}", e),
            );
            return;
        }
    };

    let session = SessionData {
        org_id,
        session_key,
        cf_clearance,
        last_active_org,
        anthropic_device_id,
        cf_bm,
        ssid,
        full_cookie_string: Some(full_cookie_string),
    };

    let path = match Config::load()
        .and_then(|config| config.session_path_for(&instance))
    {
        Ok(path) => path,
        Err(e) => {
            emit_auth_error(&app, &instance, e.to_string());
            return;
        }
    };

    if let Err(e) = session.save(&path) {
        emit_auth_error(&app, &instance, e.to_string());
        return;
    }

    if let Some(window) = app.get_webview_window("auth") {
        let _ = window.close();
    }

    let _ = app.emit("auth-complete", AuthCompletePayload { instance });
}

fn emit_auth_error(app: &AppHandle, instance: &str, message: String) {
    tracing::warn!("auth: error for '{}': {}", instance, message);
    if let Some(window) = app.get_webview_window("auth") {
        let _ = window.close();
    }
    let _ = app.emit(
        "auth-error",
        AuthErrorPayload {
            instance: instance.to_string(),
            message,
        },
    );
}

/// Enable or disable launching the app automatically when the user logs in.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

/// Return whether the app is currently registered to launch at login.
#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// Send a test notification to verify notification pipeline works.
#[tauri::command]
pub fn test_notification() -> Result<(), String> {
    use notify_rust::Notification;

    Notification::new()
        .summary("Claude Notify Test")
        .body("This is a test notification. Your notification settings are working!")
        .show()
        .map_err(|e| e.to_string())?;

    Ok(())
}
