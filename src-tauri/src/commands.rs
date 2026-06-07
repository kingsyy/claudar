use crate::monitor_loop;
use claude_notify_core::{
    config::{Config, InstanceConfig},
    history::{self, HistoryRecord},
    monitor::{poll_instance, UsagePayload},
    storage::SessionData,
    usage_fetcher::fetch_org_id,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InstanceInfo {
    pub name: String,
    /// Whether a session file exists for this instance.
    pub has_session: bool,
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
pub fn get_history(instance: Option<String>, since_days: i64) -> Result<Vec<HistoryRecord>, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    let name = instance.as_deref().unwrap_or("default");
    let records = history::load_records(&config, name).map_err(|e| e.to_string())?;

    let cutoff = Utc::now() - ChronoDuration::days(since_days);
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

/// Open an embedded login webview for the given instance, pointed at claude.ai.
///
/// Watches page loads for a successful redirect away from `/login` (the post-login
/// app shell), then extracts session cookies from the webview's cookie store,
/// resolves the organization id, saves the session, and emits `auth-complete`
/// (or `auth-error` on failure).
#[tauri::command]
pub async fn start_auth(app: AppHandle, instance: Option<String>) -> Result<(), String> {
    let instance_name = instance.unwrap_or_else(|| "default".to_string());

    if let Some(existing) = app.get_webview_window("auth") {
        let _ = existing.close();
    }

    let url = WebviewUrl::External(
        "https://claude.ai/login"
            .parse::<tauri::Url>()
            .map_err(|e| e.to_string())?,
    );

    // Guards against the navigation handler firing more than once for the same login
    // (claude.ai performs several internal redirects after a successful sign-in).
    let completed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let app_for_nav = app.clone();
    let instance_for_nav = instance_name.clone();
    let completed_for_close = completed.clone();

    let window = WebviewWindowBuilder::new(&app, "auth", url)
        .title("Log in to Claude.ai")
        .inner_size(480.0, 720.0)
        .on_page_load(move |webview, payload| {
            if payload.event() != tauri::webview::PageLoadEvent::Finished {
                return;
            }

            let url = payload.url();
            let host = url.host_str().unwrap_or("");
            let path = url.path();
            let logged_in = host.ends_with("claude.ai") && !path.starts_with("/login");

            if !logged_in {
                return;
            }

            if completed.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }

            let app = app_for_nav.clone();
            let instance_name = instance_for_nav.clone();
            let webview = webview.clone();
            tauri::async_runtime::spawn(async move {
                complete_auth(app, webview, instance_name).await;
            });
        })
        .build()
        .map_err(|e| e.to_string())?;

    // If the user closes the login window before we detect a successful sign-in,
    // surface that as an error so the wizard/banner don't stay stuck on "waiting".
    let app_for_close = app.clone();
    let instance_for_close = instance_name.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            if !completed_for_close.swap(true, std::sync::atomic::Ordering::SeqCst) {
                emit_auth_error(
                    &app_for_close,
                    &instance_for_close,
                    "Login window closed before signing in".to_string(),
                );
            }
        }
    });

    let _ = window.show();
    let _ = window.set_focus();

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
