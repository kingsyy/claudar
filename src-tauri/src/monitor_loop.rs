use crate::tauri_notifier::TauriNotificationSender;
use claudar_core::{
    config::Config,
    monitor::{poll_instance, UsagePayload},
    usage_fetcher::AuthRequiredError,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// Number of consecutive transport-level failures before escalating to `auth-required`.
/// Transient Cloudflare blips clear on their own; only persistent failures mean the
/// session has expired and the user needs to re-authenticate.
const TRANSPORT_ERROR_AUTH_THRESHOLD: u32 = 3;

/// Returns true when the error chain contains a reqwest transport failure (connect,
/// send, or timeout). These look like "error sending request for url …" and are caused
/// by Cloudflare resetting the TLS connection for expired-session requests — they are
/// *not* clean HTTP 401/403s, so they don't trip the normal AuthRequiredError path.
fn is_transport_error(e: &anyhow::Error) -> bool {
    e.chain().any(|cause| {
        cause
            .downcast_ref::<reqwest::Error>()
            .map(|re| re.is_connect() || re.is_request() || re.is_timeout())
            .unwrap_or(false)
    })
}

/// Returns a human-readable string describing which reqwest error kinds are set.
fn transport_error_kind(e: &anyhow::Error) -> String {
    for cause in e.chain() {
        if let Some(re) = cause.downcast_ref::<reqwest::Error>() {
            let mut kinds = Vec::new();
            if re.is_connect() { kinds.push("connect"); }
            if re.is_request() { kinds.push("request"); }
            if re.is_timeout() { kinds.push("timeout"); }
            if re.is_decode() { kinds.push("decode"); }
            if re.is_body() { kinds.push("body"); }
            if re.is_status() { kinds.push("status"); }
            let kind_str = if kinds.is_empty() { "unknown".to_string() } else { kinds.join("+") };
            let url_str = re.url().map(|u| u.to_string()).unwrap_or_else(|| "—".to_string());
            return format!("kind=[{}] url={}", kind_str, url_str);
        }
    }
    "no-reqwest-error-in-chain".to_string()
}

/// Tracks the running background poll task for each instance, keyed by instance name.
/// Lets `remove_instance` stop a specific instance's loop without restarting the app.
#[derive(Default)]
pub struct MonitorTasks(pub Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>);

/// Latest successful usage snapshot per instance, used to render the tray menu.
#[derive(Default)]
pub struct TrayUsage(pub Mutex<HashMap<String, UsagePayload>>);

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

    /// PNG bytes for the tray icon variant matching this level.
    fn icon_bytes(self) -> &'static [u8] {
        match self {
            Self::Grey => include_bytes!("../icons/tray-grey.png"),
            Self::Green => include_bytes!("../icons/tray-green.png"),
            Self::Yellow => include_bytes!("../icons/tray-yellow.png"),
            Self::Orange => include_bytes!("../icons/tray-orange.png"),
            Self::Red => include_bytes!("../icons/tray-red.png"),
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
        spawn_instance_task(app_handle.clone(), instance.name);
    }
}

/// Spawn a single background poll task for the given instance and register its
/// handle in `MonitorTasks` so it can be stopped later (e.g. on instance removal).
pub fn spawn_instance_task(app_handle: AppHandle, instance_name: String) {
    let task_handle = {
        let app_handle = app_handle.clone();
        let instance_name = instance_name.clone();

        tauri::async_runtime::spawn(async move {
            // Small stagger so multiple instances don't hammer the API simultaneously.
            let idx = instance_name.len() as u64 % 5;
            tokio::time::sleep(Duration::from_secs(idx)).await;

            let sender = TauriNotificationSender { app: app_handle.clone() };
            let mut consecutive_transport_errors: u32 = 0;

            loop {
                let config = Config::load().unwrap_or_default();
                let interval = Duration::from_secs(config.general.poll_interval_seconds);

                let poll_start = Instant::now();
                tracing::debug!("poll: starting request for '{}'", instance_name);
                match poll_instance(&config, &instance_name, &sender).await {
                    Ok(payload) => {
                        consecutive_transport_errors = 0;
                        tracing::info!(
                            "poll: {} — 5h={:.1}% 7d={:.1}% (took {}ms)",
                            instance_name,
                            payload.five_hour_pct,
                            payload.seven_day_pct,
                            poll_start.elapsed().as_millis()
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

                        // Record the snapshot and refresh the tray dropdown.
                        if let Some(state) = app_handle.try_state::<TrayUsage>() {
                            state
                                .0
                                .lock()
                                .unwrap()
                                .insert(instance_name.clone(), payload.clone());
                        }
                        refresh_tray_menu(&app_handle);
                    }
                    Err(e) => {
                        if e.downcast_ref::<AuthRequiredError>().is_some() {
                            consecutive_transport_errors = 0;
                            tracing::warn!(
                                "poll: auth required for '{}' (took {}ms): {:#}",
                                instance_name,
                                poll_start.elapsed().as_millis(),
                                e
                            );
                            let _ = app_handle.emit(
                                "auth-required",
                                AuthRequiredPayload { instance: instance_name.clone() },
                            );
                            set_tray_icon(&app_handle, UsageLevel::Grey);
                        } else if is_transport_error(&e) {
                            consecutive_transport_errors += 1;
                            tracing::warn!(
                                "poll: transport error {}/{} for '{}' (took {}ms) {} — chain: {:#}",
                                consecutive_transport_errors,
                                TRANSPORT_ERROR_AUTH_THRESHOLD,
                                instance_name,
                                poll_start.elapsed().as_millis(),
                                transport_error_kind(&e),
                                e
                            );
                            if consecutive_transport_errors >= TRANSPORT_ERROR_AUTH_THRESHOLD {
                                consecutive_transport_errors = 0;
                                tracing::warn!(
                                    "poll: {} consecutive transport errors for '{}' — escalating to auth-required",
                                    TRANSPORT_ERROR_AUTH_THRESHOLD,
                                    instance_name
                                );
                                let _ = app_handle.emit(
                                    "auth-required",
                                    AuthRequiredPayload { instance: instance_name.clone() },
                                );
                                set_tray_icon(&app_handle, UsageLevel::Grey);
                            } else {
                                let _ = app_handle.emit(
                                    "monitor-error",
                                    MonitorErrorPayload {
                                        instance: instance_name.clone(),
                                        message: format!(
                                            "Connection blocked by Cloudflare bot protection \
                                             (attempt {}/{}). This usually clears on its own — \
                                             if it persists, re-authenticate.\n\nDetail: {}",
                                            consecutive_transport_errors,
                                            TRANSPORT_ERROR_AUTH_THRESHOLD,
                                            e
                                        ),
                                    },
                                );
                            }
                        } else {
                            consecutive_transport_errors = 0;
                            tracing::error!(
                                "poll: error for '{}' (took {}ms): {:#}",
                                instance_name,
                                poll_start.elapsed().as_millis(),
                                e
                            );
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
        })
    };

    if let Some(state) = app_handle.try_state::<MonitorTasks>() {
        state
            .0
            .lock()
            .unwrap()
            .insert(instance_name, task_handle);
    }
}

/// Abort the running poll task for the given instance, if any, and stop tracking it.
pub fn stop_instance_task(app_handle: &AppHandle, instance_name: &str) {
    if let Some(state) = app_handle.try_state::<MonitorTasks>() {
        if let Some(handle) = state.0.lock().unwrap().remove(instance_name) {
            handle.abort();
            tracing::info!("monitor: stopped task for '{}'", instance_name);
        }
    }
}

/// Swap the tray icon to the colour variant matching the given usage level.
fn set_tray_icon(app_handle: &AppHandle, level: UsageLevel) {
    let icon = match tauri::image::Image::from_bytes(level.icon_bytes()) {
        Ok(icon) => icon,
        Err(e) => {
            tracing::warn!("decode tray icon for {:?} failed: {}", level, e);
            return;
        }
    };

    if let Some(tray) = app_handle.tray_by_id("tray") {
        if let Err(e) = tray.set_icon(Some(icon)) {
            tracing::warn!("set_tray_icon failed: {}", e);
        }
    }
}

/// Format a reset timestamp as a short "in 2h 13m" relative string for the tray menu.
fn format_resets_in(resets_at: Option<&str>) -> String {
    let Some(raw) = resets_at else {
        return "—".to_string();
    };
    let parsed = DateTime::parse_from_rfc3339(raw)
        .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", raw)));
    let Ok(reset) = parsed else {
        return "—".to_string();
    };

    let dur = reset.with_timezone(&Utc).signed_duration_since(Utc::now());
    if dur.num_seconds() <= 0 {
        "now".to_string()
    } else if dur.num_days() > 0 {
        format!("in {}d {}h", dur.num_days(), dur.num_hours() % 24)
    } else if dur.num_hours() > 0 {
        format!("in {}h {}m", dur.num_hours(), dur.num_minutes() % 60)
    } else {
        format!("in {}m", dur.num_minutes().max(1))
    }
}

/// Build the tray dropdown menu from the latest per-instance usage snapshots.
///
/// Usage rows are disabled (display-only). The actionable items reuse the ids
/// `show` and `quit`, which the tray's `on_menu_event` handler (set up in
/// `main::setup_tray`) routes — so this menu can be swapped in at any time.
pub fn build_tray_menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    usage: &HashMap<String, UsagePayload>,
) -> tauri::Result<Menu<R>> {
    let config = Config::load().unwrap_or_default();
    let instances = config.effective_instances();
    let show_names = instances.len() > 1;

    let menu = Menu::new(manager)?;
    menu.append(&MenuItem::with_id(
        manager,
        "tray_header",
        "Claudar",
        false,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(manager)?)?;

    let rendered = instances
        .iter()
        .filter_map(|inst| usage.get(&inst.name).map(|p| (inst, p)))
        .count();

    if rendered == 0 {
        menu.append(&MenuItem::with_id(
            manager,
            "tray_waiting",
            "Fetching usage…",
            false,
            None::<&str>,
        )?)?;
    } else {
        for inst in &instances {
            let Some(p) = usage.get(&inst.name) else {
                continue;
            };
            if show_names {
                menu.append(&MenuItem::with_id(
                    manager,
                    format!("tray_name_{}", inst.name),
                    &inst.name,
                    false,
                    None::<&str>,
                )?)?;
            }
            menu.append(&MenuItem::with_id(
                manager,
                format!("tray_5h_{}", inst.name),
                format!(
                    "  5-hour  {:>5.1}%  ·  resets {}",
                    p.five_hour_pct,
                    format_resets_in(p.resets_at.as_deref())
                ),
                false,
                None::<&str>,
            )?)?;
            menu.append(&MenuItem::with_id(
                manager,
                format!("tray_7d_{}", inst.name),
                format!(
                    "  7-day   {:>5.1}%  ·  resets {}",
                    p.seven_day_pct,
                    format_resets_in(p.seven_day_resets_at.as_deref())
                ),
                false,
                None::<&str>,
            )?)?;
        }
    }

    menu.append(&PredefinedMenuItem::separator(manager)?)?;
    menu.append(&MenuItem::with_id(
        manager,
        "show",
        "Open Dashboard",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        manager,
        "quit",
        "Quit",
        true,
        None::<&str>,
    )?)?;

    Ok(menu)
}

/// Rebuild the tray menu from the stored snapshots and swap it onto the live tray.
/// No-op (with a warning) if the tray or state is unavailable.
pub fn refresh_tray_menu(app_handle: &AppHandle) {
    let Some(state) = app_handle.try_state::<TrayUsage>() else {
        return;
    };
    let menu = {
        let usage = state.0.lock().unwrap();
        build_tray_menu(app_handle, &usage)
    };
    let menu = match menu {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("build_tray_menu failed: {}", e);
            return;
        }
    };
    if let Some(tray) = app_handle.tray_by_id("tray") {
        if let Err(e) = tray.set_menu(Some(menu)) {
            tracing::warn!("set_menu failed: {}", e);
        }
    }
}
