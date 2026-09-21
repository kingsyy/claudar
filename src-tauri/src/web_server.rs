use claudar_core::{
    config::Config,
    history,
    monitor::UsagePayload,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Html,
    routing::get,
    Json, Router,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

use crate::monitor_loop::TrayUsage;

/// Handle to the currently running web server task, if any. Lets us stop and
/// restart it when the user flips `web.enabled` or changes bind/port in Settings.
#[derive(Default)]
pub struct WebServerTask(pub Mutex<Option<tauri::async_runtime::JoinHandle<()>>>);

/// Stop any running dashboard server, then start a new one if `config.web.enabled`.
/// Safe to call any time — on startup and after every `set_config`.
pub fn restart_web_server(app_handle: AppHandle) {
    if let Some(state) = app_handle.try_state::<WebServerTask>() {
        if let Some(handle) = state.0.lock().unwrap().take() {
            handle.abort();
        }
    }

    let config = Config::load().unwrap_or_default();
    if !config.web.enabled {
        return;
    }

    let addr = match parse_bind_addr(&config.web.bind, config.web.port) {
        Ok(addr) => addr,
        Err(e) => {
            tracing::warn!("web dashboard: invalid bind address '{}': {}", config.web.bind, e);
            return;
        }
    };

    let handle = tauri::async_runtime::spawn(serve(app_handle.clone(), addr));

    if let Some(state) = app_handle.try_state::<WebServerTask>() {
        *state.0.lock().unwrap() = Some(handle);
    }
}

fn parse_bind_addr(bind: &str, port: u16) -> anyhow::Result<SocketAddr> {
    let ip = IpAddr::from_str(bind)?;
    Ok(SocketAddr::new(ip, port))
}

async fn serve(app_handle: AppHandle, addr: SocketAddr) {
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("web dashboard: failed to bind {}: {}", addr, e);
            return;
        }
    };

    tracing::info!("web dashboard: listening on http://{}", addr);

    let router = Router::new()
        .route("/", get(index_handler))
        .route("/healthz", get(healthz_handler))
        .route("/api/usage", get(api_usage_handler))
        .route("/api/history", get(api_history_handler))
        .route("/api/agent", get(api_agent_handler))
        .with_state(app_handle);

    if let Err(e) = axum::serve(listener, router).await {
        tracing::error!("web dashboard: server error: {}", e);
    }
}

async fn healthz_handler() -> &'static str {
    "ok"
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

/// Snapshot of every configured instance's latest known usage, in config order.
/// Instances with no successful poll yet are omitted rather than sent as nulls.
async fn api_usage_handler(State(app_handle): State<AppHandle>) -> Json<Vec<UsagePayload>> {
    let config = Config::load().unwrap_or_default();
    let instances = config.effective_instances();

    let usage = app_handle
        .try_state::<TrayUsage>()
        .map(|state| state.0.lock().unwrap().clone())
        .unwrap_or_default();

    let payloads = instances
        .into_iter()
        .filter_map(|inst| usage.get(&inst.name).cloned())
        .collect();

    Json(payloads)
}

#[derive(Deserialize)]
struct HistoryQuery {
    instance: Option<String>,
    since_days: Option<i64>,
}

/// Recent history for one instance. Returns an empty array (not an error) when
/// `history.enabled` is off, since that's an expected, common configuration.
async fn api_history_handler(
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<history::HistoryRecord>>, StatusCode> {
    let config = Config::load().unwrap_or_default();
    if !config.history.enabled {
        return Ok(Json(Vec::new()));
    }

    let name = query.instance.as_deref().unwrap_or("default");
    let since_days = query.since_days.unwrap_or(7);

    let records = history::load_records(&config, name).map_err(|e| {
        tracing::warn!("web dashboard: failed to load history for '{}': {}", name, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let cutoff = Utc::now() - ChronoDuration::days(since_days);
    Ok(Json(records.into_iter().filter(|r| r.polled_at >= cutoff).collect()))
}

const FIVE_HOUR_MINUTES: i64 = 5 * 60;
const SEVEN_DAY_MINUTES: i64 = 7 * 24 * 60;

/// A single limit window, reduced to the minimum an agent needs to decide
/// whether to throttle: usage vs. time consumed, and which way pace is drifting.
#[derive(serde::Serialize)]
struct AgentWindow {
    pct: f64,
    time_elapsed_pct: f64,
    pace: &'static str,
    resets_in_minutes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    predicted_pct: Option<f64>,
}

#[derive(serde::Serialize)]
struct AgentUsage {
    instance: String,
    /// Which service the numbers describe — an agent throttling against a
    /// ChatGPT account shouldn't have to guess.
    provider: claudar_core::config::Provider,
    /// Omitted when the provider reports no short window.
    #[serde(skip_serializing_if = "Option::is_none")]
    five_hour: Option<AgentWindow>,
    seven_day: AgentWindow,
}

fn agent_window(pct: f64, resets_at: &Option<String>, window_minutes: i64, predicted_pct: Option<f64>) -> AgentWindow {
    let (time_elapsed_pct, resets_in_minutes) = match resets_at.as_deref().and_then(|s| s.parse::<chrono::DateTime<Utc>>().ok()) {
        Some(reset) => {
            let remaining_minutes = (reset - Utc::now()).num_minutes();
            let elapsed_minutes = window_minutes - remaining_minutes;
            let time_pct = (elapsed_minutes as f64 / window_minutes as f64 * 100.0).clamp(0.0, 100.0);
            (time_pct, Some(remaining_minutes.max(0)))
        }
        None => (0.0, None),
    };

    let pace_diff = pct - time_elapsed_pct;
    let pace = if pace_diff > 10.0 {
        "over"
    } else if pace_diff < -10.0 {
        "under"
    } else {
        "on"
    };

    AgentWindow {
        pct,
        time_elapsed_pct,
        pace,
        resets_in_minutes,
        predicted_pct,
    }
}

/// Minimal, machine-readable usage snapshot for other agents/tools to poll —
/// just enough per window (usage %, pace vs. time elapsed, reset countdown,
/// predicted peak) to decide whether to throttle work, no history or HTML.
/// Gated separately from the human dashboard since it's meant for
/// programmatic/unattended access.
async fn api_agent_handler(State(app_handle): State<AppHandle>) -> Result<Json<Vec<AgentUsage>>, StatusCode> {
    let config = Config::load().unwrap_or_default();
    if !config.web.agent_api_enabled {
        return Err(StatusCode::NOT_FOUND);
    }

    let instances = config.effective_instances();
    let usage = app_handle
        .try_state::<TrayUsage>()
        .map(|state| state.0.lock().unwrap().clone())
        .unwrap_or_default();

    let payloads = instances
        .into_iter()
        .filter_map(|inst| usage.get(&inst.name).cloned())
        .map(|p| {
            let short = p.has_short_window().then(|| {
                agent_window(
                    p.five_hour_pct,
                    &p.resets_at,
                    p.five_hour_window_seconds.map_or(FIVE_HOUR_MINUTES, |s| (s / 60).max(1)),
                    p.predicted_pct,
                )
            });
            AgentUsage {
                provider: p.provider,
                five_hour: short,
                seven_day: agent_window(
                    p.seven_day_pct,
                    &p.seven_day_resets_at,
                    p.seven_day_window_seconds.map_or(SEVEN_DAY_MINUTES, |s| (s / 60).max(1)),
                    None,
                ),
                instance: p.instance,
            }
        })
        .collect();

    Ok(Json(payloads))
}
