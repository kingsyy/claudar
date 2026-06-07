use claude_notify_core::{
    config::Config,
    monitor::poll_instance,
    usage_fetcher::AuthRequiredError,
};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Colour levels used to encode worst-case usage in the tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageLevel {
    Grey,   // no data / error
    Green,  // < 50%
    Yellow, // 50–74%
    Orange, // 75–89%
    Red,    // ≥ 90%
}

impl UsageLevel {
    fn from_pct(max_pct: f64) -> Self {
        if max_pct >= 90.0 {
            Self::Red
        } else if max_pct >= 75.0 {
            Self::Orange
        } else if max_pct >= 50.0 {
            Self::Yellow
        } else {
            Self::Green
        }
    }

    fn rgba(self) -> [u8; 4] {
        match self {
            Self::Grey => [128, 128, 128, 255],
            Self::Green => [0, 180, 0, 255],
            Self::Yellow => [200, 180, 0, 255],
            Self::Orange => [220, 120, 0, 255],
            Self::Red => [220, 0, 0, 255],
        }
    }
}

#[derive(Clone, Serialize)]
struct AuthRequiredPayload {
    instance: String,
}

#[derive(Clone, Serialize)]
struct MonitorErrorPayload {
    instance: String,
    message: String,
}

/// Spawn one background tokio task per configured instance.
/// Each task polls indefinitely; errors are emitted as Tauri events, not panics.
pub fn spawn_monitor_tasks(app_handle: AppHandle) {
    let config = Config::load().unwrap_or_default();
    let instances = config.effective_instances();
    let poll_secs = config.general.poll_interval_seconds;

    tracing::info!(
        "monitor: spawning tasks for {} instance(s), interval={}s",
        instances.len(),
        poll_secs
    );

    for instance in instances {
        let app_handle = app_handle.clone();
        let instance_name = instance.name.clone();

        tauri::async_runtime::spawn(async move {
            // Small stagger so multiple instances don't hammer the API simultaneously.
            let idx = instance_name.len() as u64 % 5;
            tokio::time::sleep(Duration::from_secs(idx)).await;

            loop {
                let config = Config::load().unwrap_or_default();
                let interval = Duration::from_secs(config.general.poll_interval_seconds);

                match poll_instance(&config, &instance_name).await {
                    Ok(payload) => {
                        tracing::info!(
                            "poll: {} — 5h={:.1}% 7d={:.1}%",
                            instance_name,
                            payload.five_hour_pct,
                            payload.seven_day_pct
                        );

                        // Emit usage-update event to all subscribers (frontend).
                        if let Err(e) = app_handle.emit("usage-update", &payload) {
                            tracing::warn!("emit usage-update failed: {}", e);
                        }

                        // Reflect worst-case level in tray icon.
                        let level = UsageLevel::from_pct(
                            payload.five_hour_pct.max(payload.seven_day_pct),
                        );
                        set_tray_icon(&app_handle, level);
                    }
                    Err(e) => {
                        if e.downcast_ref::<AuthRequiredError>().is_some() {
                            tracing::warn!("poll: auth required for '{}': {}", instance_name, e);
                            let _ = app_handle.emit(
                                "auth-required",
                                AuthRequiredPayload { instance: instance_name.clone() },
                            );
                            set_tray_icon(&app_handle, UsageLevel::Grey);
                        } else {
                            tracing::error!("poll: error for '{}': {}", instance_name, e);
                            let _ = app_handle.emit(
                                "monitor-error",
                                MonitorErrorPayload {
                                    instance: instance_name.clone(),
                                    message: e.to_string(),
                                },
                            );
                        }
                    }
                }

                tokio::time::sleep(interval).await;
            }
        });
    }
}

/// Replace the tray icon with a solid 16×16 colour block.
fn set_tray_icon(app_handle: &AppHandle, level: UsageLevel) {
    let color = level.rgba();
    let rgba: Vec<u8> = std::iter::repeat(color)
        .take(16 * 16)
        .flat_map(|c| c)
        .collect();
    let icon = tauri::image::Image::new(&rgba, 16, 16);

    if let Some(tray) = app_handle.tray_by_id("tray") {
        if let Err(e) = tray.set_icon(Some(icon)) {
            tracing::warn!("set_tray_icon failed: {}", e);
        }
    }
}
