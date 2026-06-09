use claudar_core::notification_trait::NotificationSender;
use notify_rust::Timeout;
use tauri::{AppHandle, Runtime};

pub struct TauriNotificationSender<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> NotificationSender for TauriNotificationSender<R> {
    fn send(&self, summary: &str, body: &str, timeout: Timeout, sound: bool) -> anyhow::Result<()> {
        #[cfg(target_os = "macos")]
        {
            let _ = (&self.app, timeout);
            let safe_summary = summary.replace('\\', "\\\\").replace('"', "\\\"");
            let safe_body = body.replace('\\', "\\\\").replace('"', "\\\"");
            let sound_clause = if sound { " sound name \"Glass\"" } else { "" };
            let script = format!(
                "display notification \"{safe_body}\" with title \"{safe_summary}\"{sound_clause}"
            );
            let status = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .status()?;
            if !status.success() {
                anyhow::bail!("osascript exited with {status}");
            }
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            use tauri_plugin_notification::NotificationExt;
            let _ = timeout;
            let mut builder = self.app.notification().builder().title(summary).body(body);
            if sound {
                builder = builder.sound("Glass");
            }
            builder.show().map_err(|e| anyhow::anyhow!("notification: {e}"))
        }
    }
}
