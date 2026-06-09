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

- Config: `~/.config/claudar/config.toml`
- Sessions: `~/.config/claudar/sessions/{instance}.json`
- State: `~/.config/claudar/state/{instance}.json`

## Building & Testing

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo check              # Type-check without building
```

## Key Dependencies

- `headless_chrome` - Browser automation for auth and API calls
- `notify-rust` - Cross-platform desktop notifications
- `clap` - CLI argument parsing
- `serde`/`toml`/`serde_json` - Config and data serialization
- `chrono`/`chrono-tz` - Time handling with timezone support
- `tracing` - Structured logging

## Notes

- Session cookies are stored in plaintext (system keychain integration planned)
- The `NotificationSender` trait allows mocking notifications in tests
- Service installation supports macOS (launchd) and Linux (systemd)
- The `build.rs` embeds the git commit hash for version display
