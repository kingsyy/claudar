use claudar_core::notification_trait::NotificationSender;
use notify_rust::Timeout;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

/// Translate the user's configured sound name (stored macOS-style, e.g. "Glass")
/// into a value the current platform's notification backend actually understands.
///
/// The desktop notification plugin delegates to `notify-rust`, whose `sound_name`
/// means something different per OS:
/// - **macOS**: the standard system-sound names (Glass, Ping, Funk, …) — pass through.
/// - **Windows**: only the `ms-winsoundevent:*` set is valid; the macOS names don't
///   map, so any enabled sound resolves to the default toast chime.
/// - **Linux**: freedesktop sound-naming-spec ids; "message" is the generic one.
///
/// Returning `None` lets the OS play (or skip) its default — we never forward a name
/// the backend would reject.
#[allow(unused_variables)]
fn resolve_sound(name: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        Some(name.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        Some("ms-winsoundevent:Notification.Default".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        Some("message".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

pub struct TauriNotificationSender<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> NotificationSender for TauriNotificationSender<R> {
    fn send(&self, summary: &str, body: &str, timeout: Timeout, sound: Option<&str>) -> anyhow::Result<()> {
        // One path for every platform: the notification plugin sets the correct
        // app identity per OS (bundle id on macOS, AppUserModelID on Windows), so
        // notifications attribute to Claudar rather than a generic host process.
        // Desktop timeout is governed by the OS notification centre, not us.
        let _ = timeout;

        let mut builder = self.app.notification().builder().title(summary).body(body);
        if let Some(resolved) = sound.and_then(resolve_sound) {
            builder = builder.sound(resolved);
        }
        builder.show().map_err(|e| anyhow::anyhow!("notification: {e}"))
    }
}
