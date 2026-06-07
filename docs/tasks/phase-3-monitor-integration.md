# Phase 3 — Monitor Loop + Event Bus

**Depends on:** Phase 2 (Tauri app skeleton running)

## Goal

Embed the monitoring daemon inside the Tauri process. The monitor loop polls Claude.ai on its configured interval using `reqwest` (no browser process), fires native notifications when thresholds are crossed, appends history records to JSONL, and emits `usage-update` Tauri events so the frontend can subscribe. The tray icon colour updates based on live usage. After this phase the app does its core job — monitoring — without any Chromium dependency.

## In scope

- Add `history.rs` to `claude-notify-core`: a `HistoryRecord` struct (timestamp, instance, five_hour_pct, seven_day_pct) and append/load functions over a per-instance JSONL file at `sessions/{instance}.jsonl`
- In `src-tauri/main.rs`: after the Tauri builder, spawn one `monitor_loop` tokio task per configured instance, passing an `AppHandle` for event emission
- Each poll cycle in the monitor loop:
  1. `UsageFetcher::fetch()` — `reqwest` call with saved cookies (from Phase 1)
  2. On `AuthRequired` error: emit `"auth-required" { instance }` → log warning → skip this cycle, retry next interval
  3. `MonitorState::check_thresholds()` — existing logic
  4. If threshold crossed: `NotificationSender::send()` — existing logic
  5. Append `HistoryRecord` to `sessions/{instance}.jsonl`
  6. `app_handle.emit("usage-update", UsagePayload)` with `{ instance, five_hour_pct, seven_day_pct, resets_at, predicted_pct }`
- On other error: emit `"monitor-error" { instance, message }` and log via `tracing`
- Tray icon `set_icon()` called after each successful poll to reflect the worst-case usage level (green / yellow / orange / red / grey — placeholder coloured icons for now; final assets in Phase 7)
- Implement Tauri commands: `get_instances`, `get_config`, `set_config`, `get_usage` (single on-demand fetch outside the loop)
- No `headless_chrome` — all fetching is via `reqwest`

## Out of scope

- Frontend subscribing to events (Phase 4)
- `get_history` read command (Phase 6)
- `start_auth`, `add_instance`, `remove_instance`, `set_autostart` commands (Phases 5/6)
- Re-auth flow from `auth-required` event (Phase 5 — the event is emitted here; the UI response is wired in Phase 5)
- Final tray icon assets (Phase 7)

## Data flow

```
tokio task (per instance)
  UsageFetcher::fetch(cookies)           [reqwest → claude.ai API]
    → AuthRequired: emit("auth-required") + skip cycle
    → Ok(UsageData):
        MonitorState::check_thresholds() → NotificationSender::send() [if threshold crossed]
        history::append(HistoryRecord)   [writes JSONL to sessions/{instance}.jsonl]
        app_handle.emit("usage-update", UsagePayload)
        tray.set_icon(level_icon)
```

See `02-architecture.md` — "Data Flow: Polling cycle".

## Acceptance criteria

- [ ] Monitor loop starts automatically on app launch for each configured instance
- [ ] Native desktop notification fires when a usage threshold is crossed
- [ ] `sessions/{instance}.jsonl` grows with a new record on each successful poll cycle
- [ ] `usage-update` event is emitted each poll (verifiable via Tauri devtools event log)
- [ ] Tray icon changes colour based on usage level (at least two distinct states observable)
- [ ] `get_instances`, `get_config`, `set_config`, `get_usage` commands return correct data (verifiable via browser devtools `__TAURI__.core.invoke(...)`)
- [ ] A 401 or auth-blocked poll emits `auth-required`, logs a warning, and does not crash the app or the loop
- [ ] No `headless_chrome` process is spawned at any point (confirm via `ps` while the app runs)

## Design references

- See `02-architecture.md` — "Data Flow: Polling cycle", "IPC Surface"
- See `03-decisions.md` — "Daemon: Embedded tokio task", "API polling: reqwest", "Tray Icon: Status colour encoding"
