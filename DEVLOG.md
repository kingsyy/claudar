## 2026-06-07 · Phase 4 — Svelte Dashboard screen wired to live usage events

**What:** Replaced the Dashboard stub (`ui/src/routes/Dashboard.svelte`) with the first real screen. On mount it calls `get_instances` to build a per-instance tab switcher, then `get_usage` to seed initial state before the first poll completes, and subscribes to the Tauri `usage-update` / `monitor-error` events (unsubscribing in `onDestroy`). State is kept in two `$state` records keyed by instance name (`usageByInstance`, `errorByInstance`) with a `$derived` view of the selected instance. Renders two CSS conic-gradient circular gauges (5h / 7d %), human-readable reset countdowns derived from `resets_at` / `seven_day_resets_at` (re-rendered every 30s via `setInterval` so they tick down without needing a new poll event), a predicted burn-rate stat from `predicted_pct`, plus loading-spinner and error-banner states.

**Why:** Used plain `$state` records instead of a Svelte store/external module — the data is only consumed by this one screen, and Svelte 5 runes give the same reactivity with less indirection (no premature abstraction). Receiving an `monitor-error` event clears any stale error once a subsequent `usage-update` for that instance succeeds, so the banner doesn't get stuck after a transient failure.

**Next:** Phase 5 — Onboarding wizard (Welcome → account setup → thresholds → done), including the in-app `start_auth` Chrome window flow.

## 2026-06-07 · Phase 3 — Monitor loop + event bus embedded in Tauri

**What:** Wired the monitoring daemon into the Tauri process. `src-tauri/src/monitor_loop.rs` spawns one tokio task per configured instance (via `tauri::async_runtime::spawn`). Each task calls the new `poll_instance()` in core, emits `usage-update` / `auth-required` / `monitor-error` Tauri events, and updates the tray icon colour (grey/green/yellow/orange/red based on worst-case usage). Added four IPC commands: `get_instances`, `get_config`, `set_config`, `get_usage`. Instance tasks stagger their first poll by a small offset to avoid simultaneous API hits. History JSONL is always written regardless of `config.history.enabled` (GUI always needs data).

**Why:** Added `UsagePayload` + `poll_instance()` as public API to `claude-notify-core/monitor.rs` rather than duplicating the `process_limit` / notification logic in the Tauri crate. `check_usage` now returns `anyhow::Result<UsagePayload>` — the CLI path ignores the payload, the GUI path uses it for event emission. `TrayIconBuilder::with_id("tray")` gives the tray a stable ID so `monitor_loop` can call `app_handle.tray_by_id("tray")` from background tasks. Tauri 2.0 requires `use tauri::Emitter` in scope for `AppHandle::emit()` — not obvious from docs.

**Next:** Phase 4 — Svelte Dashboard subscribes to `usage-update` events and renders live 5h/7d gauges.

## 2026-06-07 · Phase 2 — Tauri 2.0 app skeleton + Svelte 5 frontend scaffold

**What:** Added `src-tauri/` (Tauri 2.11.2, `claude-notify-app` crate) and `ui/` (Svelte 5 + Vite 6 + Tailwind CSS v4 + shadcn-svelte config). Grey 16×16 RGBA tray icon, context menu (Show/Hide/Quit), left-click toggle, close-button hides window. Stub `ping` IPC command. Sidebar scaffold with four nav routes (Dashboard, History, Accounts, Settings). Builds clean; `cargo tauri dev` launches the app.

**Why:** Tauri 2.11.2 pinned explicitly — resolving `tauri = "2"` picked v2.9.5 which had a trait-mismatch bug with `wry 0.53.5` (`eval_script_with_callback` not implemented). Svelte 5 runes used for state (`$state`). `beforeDevCommand` path: Tauri sets CWD to the parent of `frontendDist` (i.e. `ui/`), so `npm run dev` works without `--prefix` — confirmed via `pwd && ls` probe. Icons regenerated as RGBA (color type 6) after `generate_context!()` rejected RGB PNGs.

**Next:** Phase 3 — wire `run_monitor()` into a Tauri tokio task, emit `usage-update` events to the Svelte frontend.
