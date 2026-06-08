use crate::{chrome_auth, monitor_loop};
use claude_notify_core::{
    config::{Config, InstanceConfig},
    history::{self, HistoryRecord},
    monitor::{poll_instance, UsagePayload},
    usage_fetcher::fetch_org_id,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
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

    match chrome_auth::run_chrome_auth(profile_path).await {
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

/// Open a magic-link URL in Chrome (the active auth session handles it via CDP).
#[tauri::command]
pub async fn navigate_auth_window(_app: AppHandle, url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())?;
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

/// Send a test notification to verify notification pipeline works.
#[tauri::command]
pub fn test_notification() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let script = r#"display notification "This is a test notification. Your notification settings are working!" with title "Claude Notify Test""#;
        let status = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to send notification".to_string());
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        use notify_rust::Notification;
        Notification::new()
            .appname("Claude Notify")
            .summary("Claude Notify Test")
            .body("This is a test notification. Your notification settings are working!")
            .show()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
