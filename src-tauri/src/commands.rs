use crate::{chrome_auth, monitor_loop, tauri_notifier::TauriNotificationSender};
use claudar_core::{
    config::{Config, InstanceConfig},
    history::{self, HistoryRecord},
    monitor::{poll_instance, UsagePayload},
    usage_fetcher::fetch_org_id,
};
use chrono::{Duration as ChronoDuration, Utc};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tokio_tungstenite::tungstenite::Message;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InstanceInfo {
    pub name: String,
    /// Whether a session file exists for this instance.
    pub has_session: bool,
}

/// Remote-debugging port of each instance's *currently running* auth Chrome,
/// keyed by instance name. Lets `navigate_auth_window` drive the same browser
/// the login flow is watching (e.g. to open a pasted magic link). Entries exist
/// only while `run_chrome_auth_and_emit` is running for that instance.
#[derive(Default)]
pub struct AuthPorts(pub Mutex<HashMap<String, u16>>);

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

/// Persist a modified config to disk, then restart the web dashboard so
/// changes to `web.enabled`/`bind`/`port` take effect without an app restart.
#[tauri::command]
pub fn set_config(app: AppHandle, _instance: Option<String>, config: Config) -> Result<(), String> {
    config.thresholds.validate()?;
    config.save().map_err(|e| e.to_string())?;
    crate::web_server::restart_web_server(app);
    Ok(())
}

/// On-demand usage fetch for a single instance (bypasses the background loop).
/// Fires desktop notifications and writes a history record just like the loop does.
#[tauri::command]
pub async fn get_usage(app: AppHandle, instance: Option<String>) -> Result<UsagePayload, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    let name = instance.as_deref().unwrap_or("default");
    let webview_fetcher = crate::webview_fetch::make_webview_fetcher(app.clone());
    let sender = TauriNotificationSender { app };
    poll_instance(&config, name, &sender, Some(&webview_fetcher))
        .await
        .map_err(|e| e.to_string())
}

/// Return history records for an instance, filtered to the last `since_days` days.
#[tauri::command]
pub fn get_history(
    instance: Option<String>,
    since_days: i64,
) -> Result<Vec<HistoryRecord>, String> {
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

/// Start Chrome-based authentication flow.
/// Opens Chrome at claude.ai/login and spawns async auth task.
#[tauri::command]
pub async fn start_auth(app: AppHandle, instance: Option<String>) -> Result<(), String> {
    let instance_name = instance.unwrap_or_else(|| "default".to_string());

    if let Some(existing) = app.get_webview_window("auth") {
        let _ = existing.close();
    }

    // Spawn background auth task
    let app_clone = app.clone();
    let name_clone = instance_name.clone();
    tauri::async_runtime::spawn(async move {
        run_chrome_auth_and_emit(app_clone, name_clone).await;
    });

    Ok(())
}

/// Run Chrome auth flow and emit completion/error event.
async fn run_chrome_auth_and_emit(app: AppHandle, instance_name: String) {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            emit_auth_error(&app, &instance_name, e.to_string());
            return;
        }
    };

    let profile_path = match config.chrome_profile_path_for(&instance_name) {
        Ok(p) => p,
        Err(e) => {
            emit_auth_error(&app, &instance_name, e.to_string());
            return;
        }
    };

    // Reserve the debug port up front and publish it so `navigate_auth_window`
    // can reach this Chrome; deregister on every exit path via the guard.
    let debug_port = match chrome_auth::find_free_port() {
        Ok(p) => p,
        Err(e) => {
            emit_auth_error(&app, &instance_name, e.to_string());
            return;
        }
    };
    app.state::<AuthPorts>()
        .0
        .lock()
        .unwrap()
        .insert(instance_name.clone(), debug_port);
    let _port_guard = AuthPortGuard {
        app: app.clone(),
        instance: instance_name.clone(),
    };

    match chrome_auth::run_chrome_auth(profile_path, debug_port).await {
        Ok(mut session) => {
            match fetch_org_id(
                session
                    .full_cookie_string
                    .as_deref()
                    .unwrap_or(""),
                session.last_active_org.as_deref(),
            )
            .await
            {
                Ok(org_id) => {
                    session.org_id = org_id;
                    let path = match Config::load()
                        .and_then(|c| c.session_path_for(&instance_name))
                    {
                        Ok(p) => p,
                        Err(e) => {
                            emit_auth_error(&app, &instance_name, e.to_string());
                            return;
                        }
                    };

                    if let Err(e) = session.save(&path) {
                        emit_auth_error(&app, &instance_name, e.to_string());
                        return;
                    }

                    let _ = app.emit(
                        "auth-complete",
                        AuthCompletePayload {
                            instance: instance_name,
                        },
                    );
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    return;
                }
                Err(e) => {
                    emit_auth_error(
                        &app,
                        &instance_name,
                        format!("Logged in but couldn't fetch org: {}", e),
                    );
                    return;
                }
            }
        }
        Err(e) => {
            emit_auth_error(&app, &instance_name, e.to_string());
            return;
        }
    };
}

/// Removes an instance's auth port from `AuthPorts` when the auth task ends,
/// however it exits (success, error, or early return).
struct AuthPortGuard {
    app: AppHandle,
    instance: String,
}

impl Drop for AuthPortGuard {
    fn drop(&mut self) {
        if let Some(state) = self.app.try_state::<AuthPorts>() {
            state.0.lock().unwrap().remove(&self.instance);
        }
    }
}

/// Open a magic-link URL in the *Claudar-controlled* Chrome (not the user's
/// default browser) by driving it over CDP. The login flow's cookie poll then
/// picks up the resulting `sessionKey`.
#[tauri::command]
pub async fn navigate_auth_window(
    app: AppHandle,
    instance: Option<String>,
    url: String,
) -> Result<(), String> {
    let instance_name = instance.unwrap_or_else(|| "default".to_string());

    let port = app
        .state::<AuthPorts>()
        .0
        .lock()
        .unwrap()
        .get(&instance_name)
        .copied();
    let port = port.ok_or("No active login window — click 'Open login window' first.")?;

    let ws_url = chrome_auth::get_page_ws_url(port)
        .await
        .map_err(|e| e.to_string())?;
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| format!("CDP connect failed: {}", e))?;
    let (mut ws_write, _ws_read) = ws_stream.split();

    ws_write
        .send(Message::Text(
            json!({"id": 1, "method": "Page.enable", "params": {}}).to_string(),
        ))
        .await
        .map_err(|e| e.to_string())?;
    ws_write
        .send(Message::Text(
            json!({"id": 2, "method": "Page.navigate", "params": {"url": url}}).to_string(),
        ))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn emit_auth_error(app: &AppHandle, instance: &str, message: String) {
    tracing::warn!("auth: error for '{}': {}", instance, message);
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

/// Show or hide the menu-bar / system-tray icon at runtime to match the
/// `general.show_tray_icon` setting. Persisting the config is the caller's job.
#[tauri::command]
pub fn set_tray_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("tray") {
        tray.set_visible(visible).map_err(|e| e.to_string())?;
        if visible {
            // Make sure a freshly-shown icon carries the latest usage rows.
            monitor_loop::refresh_tray_menu(&app);
        }
    }
    Ok(())
}

/// Basic build/app metadata shown in the About panel.
#[derive(Debug, Serialize)]
pub struct AboutInfo {
    pub version: &'static str,
    pub git_hash: &'static str,
    pub license: &'static str,
    pub repository: &'static str,
}

/// Return static version / license / repository info for the About panel.
#[tauri::command]
pub fn about_info() -> AboutInfo {
    AboutInfo {
        version: env!("CARGO_PKG_VERSION"),
        git_hash: env!("GIT_HASH"),
        license: "MIT",
        repository: "https://github.com/kingsyy/claudar",
    }
}

/// Open a URL in the user's default browser. Only `https://` links are allowed
/// so the renderer can't drive arbitrary command execution.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https URLs are allowed".into());
    }

    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");

    cmd.arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Send a test notification to verify notification pipeline works.
#[tauri::command]
pub fn test_notification(app: AppHandle) -> Result<(), String> {
    use claudar_core::notification_trait::NotificationSender;
    use notify_rust::Timeout;
    let config = Config::load().map_err(|e| e.to_string())?;
    let sound = config
        .notifications
        .sound
        .then_some(config.notifications.sound_name.as_str());
    let sender = TauriNotificationSender { app };
    sender
        .send(
            "Claudar Test",
            "This is a test notification. Your notification settings are working!",
            Timeout::Milliseconds(10000),
            sound,
        )
        .map_err(|e| e.to_string())
}
