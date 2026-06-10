#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chrome_auth;
mod commands;
mod monitor_loop;
mod tauri_notifier;

use claudar_core::config::Config;
use std::collections::HashMap;
use tauri::{tray::TrayIconBuilder, Manager};

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
            None,
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
        .setup(|app| {
            setup_tray(app)?;
            setup_window(app)?;

            // Spawn one background polling task per configured instance.
            monitor_loop::spawn_monitor_tasks(app.handle().clone());

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
    let win = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let _ = win.hide();
            api.prevent_close();
        }
    });
    Ok(())
}
