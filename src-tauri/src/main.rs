#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chrome_auth;
mod commands;
mod monitor_loop;
mod tauri_notifier;
mod web_server;
mod webview_fetch;

use claudar_core::config::Config;
use std::collections::HashMap;
use tauri::{tray::TrayIconBuilder, Manager};

/// CLI flag the autostart plugin appends to the login-launch command. Its
/// presence at startup means we were opened at login rather than by hand, which
/// is what gates the "start minimized" behaviour.
const LOGIN_LAUNCH_FLAG: &str = "--minimized";

/// True when this process was launched at login (i.e. carries [`LOGIN_LAUNCH_FLAG`]).
fn launched_at_login() -> bool {
    std::env::args().any(|a| a == LOGIN_LAUNCH_FLAG)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![LOGIN_LAUNCH_FLAG]),
        ))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::SIZE,
                )
                .build(),
        )
        .manage(monitor_loop::MonitorTasks::default())
        .manage(monitor_loop::TrayUsage::default())
        .manage(commands::AuthPorts::default())
        .manage(web_server::WebServerTask::default())
        .setup(|app| {
            // If opened at login and the user asked to start minimized, hide the
            // window as early as possible to avoid a visible flash. Manual launches
            // (no login flag) always show the window regardless of the setting.
            let start_minimized = Config::load()
                .map(|c| c.general.start_minimized)
                .unwrap_or(false);
            if start_minimized && launched_at_login() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            setup_tray(app)?;
            setup_window(app)?;

            // Spawn one background polling task per configured instance.
            monitor_loop::spawn_monitor_tasks(app.handle().clone());

            // Optional read-only HTTP dashboard (off by default, see config.web).
            web_server::restart_web_server(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_instances,
            commands::get_config,
            commands::set_config,
            commands::get_usage,
            commands::get_history,
            commands::add_instance,
            commands::remove_instance,
            commands::start_auth,
            commands::navigate_auth_window,
            commands::set_autostart,
            commands::get_autostart,
            commands::test_notification,
            commands::set_tray_visible,
            commands::about_info,
            commands::open_url,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, _event| {
            // On macOS, clicking the dock icon while the window is hidden (e.g.
            // after a minimized login launch or a close-to-tray) re-opens it.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                if let Some(window) = _app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}

fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Initial menu (no usage yet); monitor_loop swaps in a populated one after
    // the first poll. Usage rows are display-only; "show"/"quit" are actioned
    // by the `on_menu_event` handler below.
    let menu = monitor_loop::build_tray_menu(app, &HashMap::new())?;

    // Start with the grey "no data yet" variant — monitor_loop swaps in the
    // colour matching current usage once the first poll completes.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-grey.png"))?;

    // ID "tray" is used by monitor_loop::{set_tray_icon, refresh_tray_menu} to
    // locate this handle. Left-click opens the usage dropdown.
    let tray = TrayIconBuilder::with_id("tray")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    // The icon is opt-in (config `general.show_tray_icon`, default off).
    let show_tray = Config::load()
        .map(|c| c.general.show_tray_icon)
        .unwrap_or(false);
    tray.set_visible(show_tray)?;

    Ok(())
}

fn setup_window(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    // The window-state plugin restores the last saved position. If that position
    // is now off-screen (monitor unplugged, resolution changed, etc.) the window
    // can land almost entirely outside the visible area. Recenter when too little
    // of it overlaps any monitor.
    ensure_on_screen(&window);

    let win = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let _ = win.hide();
            api.prevent_close();
        }
    });
    Ok(())
}

/// Recenter the window if less than a usable slice of it overlaps any monitor.
fn ensure_on_screen(window: &tauri::WebviewWindow) {
    // Minimum visible width/height (in physical pixels) we require to consider
    // the window reachable — enough to grab the title bar and drag it back.
    const MIN_VISIBLE: i64 = 120;

    let (Ok(pos), Ok(size), Ok(monitors)) = (
        window.outer_position(),
        window.outer_size(),
        window.available_monitors(),
    ) else {
        return;
    };

    let (wx, wy) = (pos.x as i64, pos.y as i64);
    let (ww, wh) = (size.width as i64, size.height as i64);

    let visible_enough = monitors.iter().any(|m| {
        let mp = m.position();
        let ms = m.size();
        let (mx, my) = (mp.x as i64, mp.y as i64);
        let (mw, mh) = (ms.width as i64, ms.height as i64);

        let overlap_x = (wx + ww).min(mx + mw) - wx.max(mx);
        let overlap_y = (wy + wh).min(my + mh) - wy.max(my);

        overlap_x >= MIN_VISIBLE.min(ww) && overlap_y >= MIN_VISIBLE.min(wh)
    });

    if !visible_enough {
        let _ = window.center();
    }
}
