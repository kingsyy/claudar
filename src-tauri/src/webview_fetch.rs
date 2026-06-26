use anyhow::Result;
use claudar_core::usage_fetcher::HttpFetcher;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Listener};
use tokio::sync::oneshot;

static FETCH_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
struct FetchRequest {
    id: u64,
    url: String,
    cookie: String,
}

#[derive(Clone, Deserialize)]
struct FetchResponse {
    id: u64,
    status: Option<u16>,
    body: Option<String>,
    error: Option<String>,
}

/// Ask the Tauri webview to perform an HTTP GET via its WKWebView / URLSession
/// network stack, bypassing the native-tls reqwest TLS fingerprint.
///
/// Emits `"webview-fetch-request"` to the frontend, waits up to 30 s for a
/// `"webview-fetch-response"` carrying the matching id, then returns
/// `(status_code, body)`.  Requires the webview window to be alive (it always
/// is for a tray app while the process is running).
pub async fn fetch_via_webview(app: AppHandle, url: String, cookie: String) -> Result<(u16, String)> {
    let id = FETCH_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = oneshot::channel::<FetchResponse>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let tx_clone = tx.clone();

    let listener_id = app.listen("webview-fetch-response", move |event| {
        if let Ok(resp) = serde_json::from_str::<FetchResponse>(event.payload()) {
            if resp.id == id {
                if let Some(sender) = tx_clone.lock().unwrap().take() {
                    sender.send(resp).ok();
                }
            }
        }
    });

    app.emit("webview-fetch-request", FetchRequest { id, url, cookie })?;

    let resp = tokio::time::timeout(Duration::from_secs(30), rx)
        .await
        .map_err(|_| anyhow::anyhow!("webview fetch timeout after 30s"))??;

    app.unlisten(listener_id);

    if let Some(err) = resp.error {
        anyhow::bail!("webview fetch error: {}", err);
    }

    Ok((resp.status.unwrap_or(0), resp.body.unwrap_or_default()))
}

/// Build an [`HttpFetcher`] that routes requests through the Tauri webview.
/// Clone the `AppHandle` once and capture it in the returned closure.
pub fn make_webview_fetcher(app: AppHandle) -> HttpFetcher {
    Arc::new(move |url: String, cookie: String| -> Pin<Box<dyn std::future::Future<Output = Result<(u16, String)>> + Send>> {
        let app = app.clone();
        Box::pin(fetch_via_webview(app, url, cookie))
    })
}
