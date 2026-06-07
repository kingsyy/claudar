## 2026-06-07 · Phase 3 — Monitor loop + event bus embedded in Tauri

**What:** Wired the monitoring daemon into the Tauri process. `src-tauri/src/monitor_loop.rs` spawns one tokio task per configured instance (via `tauri::async_runtime::spawn`). Each task calls the new `poll_instance()` in core, emits `usage-update` / `auth-required` / `monitor-error` Tauri events, and updates the tray icon colour (grey/green/yellow/orange/red based on worst-case usage). Added four IPC commands: `get_instances`, `get_config`, `set_config`, `get_usage`. Instance tasks stagger their first poll by a small offset to avoid simultaneous API hits. History JSONL is always written regardless of `config.history.enabled` (GUI always needs data).

**Why:** Added `UsagePayload` + `poll_instance()` as public API to `claude-notify-core/monitor.rs` rather than duplicating the `process_limit` / notification logic in the Tauri crate. `check_usage` now returns `anyhow::Result<UsagePayload>` — the CLI path ignores the payload, the GUI path uses it for event emission. `TrayIconBuilder::with_id("tray")` gives the tray a stable ID so `monitor_loop` can call `app_handle.tray_by_id("tray")` from background tasks. Tauri 2.0 requires `use tauri::Emitter` in scope for `AppHandle::emit()` — not obvious from docs.

**Next:** Phase 4 — Svelte Dashboard subscribes to `usage-update` events and renders live 5h/7d gauges.

## 2026-06-07 · Phase 2 — Tauri 2.0 app skeleton + Svelte 5 frontend scaffold

**What:** Added `src-tauri/` (Tauri 2.11.2, `claude-notify-app` crate) and `ui/` (Svelte 5 + Vite 6 + Tailwind CSS v4 + shadcn-svelte config). Grey 16×16 RGBA tray icon, context menu (Show/Hide/Quit), left-click toggle, close-button hides window. Stub `ping` IPC command. Sidebar scaffold with four nav routes (Dashboard, History, Accounts, Settings). Builds clean; `cargo tauri dev` launches the app.

**Why:** Tauri 2.11.2 pinned explicitly — resolving `tauri = "2"` picked v2.9.5 which had a trait-mismatch bug with `wry 0.53.5` (`eval_script_with_callback` not implemented). Svelte 5 runes used for state (`$state`). `beforeDevCommand` path: Tauri sets CWD to the parent of `frontendDist` (i.e. `ui/`), so `npm run dev` works without `--prefix` — confirmed via `pwd && ls` probe. Icons regenerated as RGBA (color type 6) after `generate_context!()` rejected RGB PNGs.

**Next:** Phase 3 — wire `run_monitor()` into a Tauri tokio task, emit `usage-update` events to the Svelte frontend.
