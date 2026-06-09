# 02 — Architecture

---

## Components

| Component | Responsibility |
|-----------|---------------|
| `claudar-core` (Rust lib) | All existing logic: monitor loop, usage fetcher (reqwest), config, history, storage, notifications, state |
| `claudar` (CLI bin) | Existing CLI surface — kept for power users and debugging; thin shell over core |
| `claudar-app` (Tauri bin) | Tauri entry point: starts webview, registers commands, spawns monitor task, manages tray |
| Svelte 5 frontend | All UI: onboarding wizard, dashboard, history, settings, accounts |
| System tray | TrayIcon with status icon + context menu (Show Window, Open at Login toggle, Quit) |

---

## Process Model

```
claudar-app (single OS process)
├── main thread: Tauri event loop
│   ├── WebviewWindow "main"   ←→  Svelte frontend
│   └── TrayIcon
└── tokio runtime
    ├── task: monitor_loop (one per instance)
    │         polls API every N min
    │         emits "usage-update" Tauri event → frontend
    │         fires native notifications (notify-rust)
    └── task: history_writer
              appends HistoryRecord → JSONL on disk
```

The monitor loop runs in the background whether the window is open or not. The window simply subscribes to events when visible.

---

## Screens & Navigation

```
App launch
  └── first run? ──yes──► Onboarding Wizard (full-window)
                             Step 1: Welcome
                             Step 2: Account setup (Chrome launches here)
                             Step 3: Configure thresholds
                             Step 4: Done / open at login prompt
        │
       no
        │
        ▼
  Main Window (sidebar layout)
  ├── Dashboard     live 5h + 7d gauges, reset countdowns, per-instance tabs
  ├── History       time-series chart, window summaries, predicted burn-rate
  ├── Accounts      list instances, add (triggers mini onboarding), remove
  └── Settings      polling interval, thresholds, notifications, open-at-login toggle
```

Window is hidden (not closed) when user clicks X — tray icon remains active. Re-opening via tray or Dock re-shows it.

---

## Data Flow

### Polling cycle (every N minutes per instance)

```
monitor_loop task
  1. UsageFetcher::fetch()                 [reqwest + saved session cookies → claude.ai API]
       → on 401/block: emit("auth-required", { instance }) → UI prompts re-auth
  2. MonitorState::check_thresholds()      [existing state.rs]
  3. → if threshold crossed: NotificationSender::send()  [existing notifications.rs]
  4. HistoryRecord written → sessions/{instance}.jsonl   [new history.rs]
  5. app.emit("usage-update", UsagePayload)              [Tauri event]
                                    │
                                    ▼
                            Svelte frontend
                            updates Dashboard gauges reactively
```

### Onboarding auth flow

```
Svelte "Step 2: Connect account"
  → invoke("start_auth", { instance })   [Tauri command]
       │
       ▼ Rust (src-tauri/commands.rs)
  WebviewWindowBuilder::new("auth", "https://claude.ai")
  → window opens inside the app (not Chrome)
  → Tauri cookie listener detects claude.ai session cookie set
  → cookies extracted via Tauri cookie API
  → session saved to sessions/{instance}.json
  → auth window closed
  → emit("auth-complete", { instance })
       │
       ▼ Svelte
  Wizard advances to Step 3
```

### History query

```
Svelte History view mounts
  → invoke("get_history", { instance, since_days: 7 })
       │
       ▼ Rust
  history::load_records(instance, since)   [reads JSONL]
  → returns Vec<HistoryRecord> as JSON
       │
       ▼ Svelte
  LayerChart renders area series
```

---

## IPC Surface (Tauri Commands)

| Command | Direction | Payload |
|---------|-----------|---------|
| `get_instances` | JS→Rust | → `Vec<InstanceInfo>` |
| `get_usage` | JS→Rust | `instance` → `UsagePayload` |
| `get_history` | JS→Rust | `instance, since_days` → `Vec<HistoryRecord>` |
| `start_auth` | JS→Rust | `instance` → starts Chrome auth flow |
| `get_config` | JS→Rust | `instance` → `Config` |
| `set_config` | JS→Rust | `instance, Config` → saves |
| `add_instance` | JS→Rust | `name` → creates new instance |
| `remove_instance` | JS→Rust | `instance` → deletes session + state |
| `set_autostart` | JS→Rust | `enabled: bool` → configures autostart plugin |
| `get_autostart` | JS→Rust | → `bool` |

| Event (Rust→JS) | Payload |
|-----------------|---------|
| `usage-update` | `{ instance, five_hour_pct, seven_day_pct, resets_at, predicted_pct }` |
| `auth-complete` | `{ instance }` |
| `auth-error` | `{ instance, message }` |
| `monitor-error` | `{ instance, message }` |
| `auth-required` | `{ instance }` — session expired or Cloudflare blocked; prompt re-auth |

---

## Cargo Workspace Layout

```
claudar/
├── Cargo.toml                  # [workspace] members = [...]
├── crates/
│   └── claudar-core/     # lib crate — all existing src/ modules
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── browser_auth.rs
│           ├── config.rs
│           ├── history.rs
│           ├── monitor.rs      # run_monitor() becomes async fn called by Tauri task
│           ├── notifications.rs
│           └── ... (rest of existing modules)
├── src/                        # existing CLI bin (thin wrapper)
│   └── main.rs
├── Cargo.toml                  # existing bin manifest → points to core lib
└── src-tauri/                  # NEW: Tauri app
    ├── Cargo.toml              # depends on claudar-core
    ├── src/
    │   ├── main.rs             # Tauri entry, register commands, spawn monitor task
    │   └── commands.rs         # all #[tauri::command] fns
    ├── tauri.conf.json
    └── icons/
ui/                             # NEW: Svelte 5 frontend
├── src/
│   ├── App.svelte
│   ├── routes/
│   │   ├── Dashboard.svelte
│   │   ├── History.svelte
│   │   ├── Accounts.svelte
│   │   └── Settings.svelte
│   └── onboarding/
│       └── Wizard.svelte
├── package.json
└── vite.config.ts
```

---

## Out of Scope for MVP

- Keychain integration (sessions stored in plaintext as today)
- Multiple simultaneous history charts (one instance at a time)
- CLI command parity via GUI (CLI stays as-is)
- Windows-specific Chrome detection/installation prompt
- In-app update mechanism
- Dark/light mode theming (system default only)
