//! In-app updates.
//!
//! Claudar checks GitHub Releases for a newer version and can install it from
//! Settings → About. The update feed is the `latest.json` that CI publishes
//! alongside the release bundles; `tauri.conf.json` points `plugins.updater` at
//! it and holds the public half of the minisign key that signs it.
//!
//! The plugin's JS API is deliberately unused — the rest of this app talks to
//! the backend through `invoke`, so the two commands below keep that shape and
//! spare the frontend an extra npm dependency.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

const REPO: &str = "https://github.com/kingsyy/claudar";

/// What the About panel needs to describe an available update.
#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    /// The running version, so the UI can render "0.4.6 → 0.4.7" without a
    /// second round-trip.
    pub current_version: String,
    pub version: Option<String>,
    /// Release notes for the *newest* release only — not the span since
    /// `current_version`. Someone updating from several versions back will not
    /// see the intermediate releases here; `changes_url` covers that gap.
    pub notes: Option<String>,
    /// RFC 3339 publication date. The UI shows this rather than a "N releases
    /// behind" count: published versions have skipped numbers (0.4.3 and 0.4.5
    /// were never tagged), so any count derived from version numbers would lie.
    pub pub_date: Option<String>,
    /// GitHub compare view between the running and available versions, covering
    /// every change in the gap that `notes` omits.
    pub changes_url: Option<String>,
}

impl UpdateInfo {
    fn up_to_date(current_version: String) -> Self {
        Self {
            available: false,
            current_version,
            version: None,
            notes: None,
            pub_date: None,
            changes_url: None,
        }
    }
}

/// Ask the update endpoint whether a newer release exists.
///
/// Returns `available: false` rather than an error when the app is current, so
/// the UI can tell "you're up to date" apart from "the check failed".
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let current_version = app.package_info().version.to_string();

    let updater = app
        .updater()
        .map_err(|e| format!("could not reach the update service: {e}"))?;

    match updater.check().await {
        Ok(Some(update)) => {
            let changes_url = format!(
                "{REPO}/compare/v{}...v{}",
                update.current_version, update.version
            );
            Ok(UpdateInfo {
                available: true,
                current_version: update.current_version.clone(),
                version: Some(update.version.clone()),
                notes: update.body.clone(),
                pub_date: update.date.map(|d| d.to_string()),
                changes_url: Some(changes_url),
            })
        }
        Ok(None) => Ok(UpdateInfo::up_to_date(current_version)),
        Err(e) => Err(format!("update check failed: {e}")),
    }
}

/// Download the available update, install it, and relaunch.
///
/// Re-runs the check instead of caching the `Update` from `check_update`: the
/// extra request costs nothing next to the download, and it avoids holding
/// cross-command state that could go stale while the user reads the notes.
///
/// This does not return on success — the app is replaced and restarted.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("could not reach the update service: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("update check failed: {e}"))?
        .ok_or_else(|| "no update available".to_string())?;

    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| format!("update failed: {e}"))?;

    // Windows hands control to the installer, which terminates the app itself;
    // on macOS and Linux the bundle is swapped in place and we relaunch into it.
    app.restart();
}
