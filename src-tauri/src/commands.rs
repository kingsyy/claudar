use claude_notify_core::{
    config::Config,
    monitor::{poll_instance, UsagePayload},
};
use serde::Serialize;

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
