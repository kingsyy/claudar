# Claudar - Developer Guide

## Project Overview

A Rust CLI daemon that monitors Claude.ai usage limits by fetching real usage data via authenticated browser requests and sending native desktop notifications.

## Architecture

```
src/
├── main.rs              # Entry point, command routing
├── browser_auth.rs      # Headless Chrome automation (cookie injection, API fetching)
├── cli.rs               # CLI argument definitions (clap)
├── config.rs            # Configuration management (TOML)
├── config_cmd.rs        # Config CLI commands (list/get/set) and instance management
├── monitor.rs           # Core monitoring daemon with polling loop
├── notification_trait.rs # NotificationSender trait + mock for testing
├── notifications.rs     # Notification formatting and sending (notify-rust)
├── retry.rs             # Retry logic with exponential backoff
├── service.rs           # Service installation (launchd/systemd)
├── setup.rs             # Interactive setup wizard
├── state.rs             # Monitor state tracking (notification history)
├── storage.rs           # Session data persistence (JSON)
├── status.rs            # Service status display
├── time_format.rs       # Timezone-aware time formatting
└── usage.rs             # Usage display with progress bars
```

## Key Concepts

- **Instances**: Support for multiple Claude accounts. Each instance has its own session and state files in `sessions/` and `state/` subdirectories.
- **Headless Chrome**: Used for both authentication (setup) and API polling. Bypasses Cloudflare bot detection by using a real browser context.
- **State Tracking**: `MonitorState` tracks which notifications have been sent to prevent spam. State resets when usage limits reset.

## Data Storage

Paths are resolved via `dirs::config_dir()` (see `Config::config_dir`), so the base
directory is platform-specific — **not** always `~/.config`:

- macOS: `~/Library/Application Support/claudar/`
- Linux: `~/.config/claudar/` (or `$XDG_CONFIG_HOME/claudar/`)
- Windows: `%APPDATA%\claudar\` (e.g. `C:\Users\<user>\AppData\Roaming\claudar\`)

Under that base: `config.toml`, `sessions/{instance}.json`, `state/{instance}.json`,
`history/{instance}.jsonl`, and `chrome-profiles/{instance}/` (auth login profiles).

## Building & Testing

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo check              # Type-check without building
```

## Key Dependencies

- Browser automation for auth: the Tauri app spawns a Chromium-based browser and drives it directly over the Chrome DevTools Protocol (`tokio-tungstenite` WebSocket + `reqwest`); no `headless_chrome` crate
- `tauri-plugin-notification` / `notify-rust` - Cross-platform desktop notifications
- `clap` - CLI argument parsing
- `serde`/`toml`/`serde_json` - Config and data serialization
- `chrono`/`chrono-tz` - Time handling with timezone support
- `tracing` - Structured logging

## Notes

- Session files are encrypted at rest (AES-256-GCM, see `crates/claudar-core/src/crypto.rs`); the key lives in the OS keychain, with a `0600` key-file fallback if no keychain backend is available. Keychain access is scoped to a **single** named item (service `claudar`, account `session-encryption-key`) — Claudar never enumerates or reads any other keychain entry. The OS may prompt for permission on first access; the prompt names this item so users can see exactly what's being requested.
- Browser detection for the login flow (`src-tauri/src/chrome_auth.rs`) is one code path across OSes: it checks well-known install locations (machine-wide *and* per-user) for Chrome/Edge/Brave/Chromium, then falls back to a `$PATH` search. Chrome is only needed for one-time login — ongoing polling uses `reqwest` with stored cookies, no browser required.
- Desktop notifications from the Tauri app go through a single path (`tauri-plugin-notification` → `notify-rust`) on all platforms; sound names are translated per-OS in `tauri_notifier::resolve_sound`. (The standalone CLI's `RealNotificationSender` still uses `osascript` on macOS, which is correct for an unbundled binary.)
- The `NotificationSender` trait allows mocking notifications in tests
- Service installation supports macOS (launchd) and Linux (systemd)
- The `build.rs` embeds the git commit hash for version display
