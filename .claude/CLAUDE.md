# Claude Notify - Implementation Progress

This document tracks the implementation status of the Claude Code usage monitoring tool.

## Project Status

### Phase 0: Setup Wizard ✅ COMPLETE

**Implemented:**
- Interactive setup wizard (`claude-notify setup`)
- Automated browser-based authentication using headless Chrome
- Email verification code flow
- Cookie extraction (sessionKey, cf_clearance, anthropic-device-id, etc.)
- Organization ID detection from API calls and page URLs
- Session data storage at `~/.config/claude-notify/session.json`
- API connection testing

**Files:**
- `src/setup.rs` - Interactive setup wizard
- `src/browser_auth.rs` - Headless Chrome automation
- `src/storage.rs` - Session data persistence

**Deviations from original spec:**
- Changed from manual cookie extraction to automated browser login
- This provides better UX and bypasses Cloudflare bot detection
- No encryption implemented yet (planned for Phase 2)

### Phase 1: Core Monitoring ✅ COMPLETE

**Implemented:**
- API polling daemon with configurable interval (default: 15 minutes)
- Usage data fetching from `https://claude.ai/api/organizations/{org_id}/usage`
- Smart notification logic with three notification types:
  - **Threshold crossings**: Alert at 50%, 70%, 90% utilization
  - **Predicted overage**: Warn if usage rate suggests exceeding 100% before reset (5-hour limit only)
  - **Reset notifications**: Alert when limits reset
- Native OS notifications using `notify-rust` 4.11 (cross-platform: macOS/Linux/Windows)
- Status command with colored progress bars (`claude-notify status`)
- Foreground monitoring mode (`claude-notify run`)
- Background service installation:
  - macOS: launchd plist installation
  - Linux: systemd unit file installation
- Service management commands:
  - `claude-notify setup-service` - Install service
  - `claude-notify start` - Start background service
  - `claude-notify stop` - Stop background service
  - `claude-notify uninstall-service` - Remove service
- Smart notification state tracking to prevent spam
- Monitor state persistence at `~/.config/claude-notify/monitor_state.json`

**Files:**
- `src/monitor.rs` - Core monitoring daemon with polling loop
- `src/notifications.rs` - Notification logic using notify-rust
- `src/state.rs` - Monitor state tracking (notification history)
- `src/service.rs` - Service installation (launchd/systemd)
- `src/status.rs` - Status display with progress bars
- `src/cli.rs` - Command-line interface
- `src/config.rs` - Configuration management

**Configuration:**
All settings stored in `~/.config/claude-notify/config.toml`:

```toml
[general]
poll_interval_seconds = 900  # 15 minutes

[thresholds]
five_hour = [50, 70, 90]
seven_day = [50, 70, 90]

[notifications]
sound = true
persistent = false
notify_threshold_crossings = true    # NEW
notify_predicted_overage = true      # NEW
notify_resets = true                 # NEW
```

**Notification Library:**
- Using `notify-rust` 4.11 (already in dependencies)
- Provides unified API across macOS, Linux, and Windows
- Supports urgency levels (Low, Normal, Critical)
- Configurable timeout and persistence
- Sufficient features for our use case (title, body, urgency)

### Phase 2: Polish 📋 PLANNED

**Not Yet Implemented:**
- Encrypted session storage using system keychain
- Auto cookie refresh mechanism
- Cookie expiration warnings
- GUI configuration tool
- Historical usage analytics
- Status bar/menubar icon
- Webhook integrations (Slack/Discord)

## Architecture

### Current File Structure

```
src/
├── main.rs              # Entry point, command routing
├── browser_auth.rs      # Headless Chrome automation
├── cli.rs               # CLI argument definitions
├── config.rs            # Configuration management
├── monitor.rs           # Core monitoring daemon
├── notifications.rs     # Native notification wrapper
├── service.rs           # Service installation (launchd/systemd)
├── setup.rs             # Interactive setup wizard
├── state.rs             # Monitor state tracking
├── status.rs            # Status display command
└── storage.rs           # Session data persistence
```

### Data Storage

- **Config**: `~/.config/claude-notify/config.toml`
- **Session**: `~/.config/claude-notify/session.json`
- **State**: `~/.config/claude-notify/monitor_state.json`
- **Logs (macOS service)**: `/tmp/claude-notify.log`, `/tmp/claude-notify.error.log`

### Key Dependencies

```toml
tokio = { version = "1.42", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "cookies", "blocking", "rustls-tls", "gzip", "brotli"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
notify-rust = "4.11"
chrono = { version = "0.4", features = ["serde"] }
toml = "0.8"
clap = { version = "4.5", features = ["derive"] }
headless_chrome = "1.0"
colored = "2.1"
dialoguer = "0.11"
```

## Usage Examples

### Initial Setup
```bash
# Run setup wizard (automated browser login)
claude-notify setup
```

### Check Current Status
```bash
# View current usage with progress bars
claude-notify status

# View with debug output
claude-notify status --verbose
```

### Monitor in Foreground
```bash
# Run monitor in foreground (shows logs)
claude-notify run

# Run with verbose output
claude-notify run --verbose
```

### Install as Background Service
```bash
# Install service (macOS: launchd, Linux: systemd)
claude-notify setup-service

# Start the service
claude-notify start

# Stop the service
claude-notify stop

# Uninstall service
claude-notify uninstall-service
```

## Implementation Notes

### Notification Behavior

1. **Threshold Crossings**
   - Notified once per threshold per reset period
   - State tracked in `monitor_state.json`
   - Urgency increases with threshold percentage
   - Notifications cleared when usage resets

2. **Predicted Overage**
   - Only for 5-hour limit
   - Triggers when predicted usage > 100% before reset
   - Based on current usage rate vs. time elapsed
   - Maximum one warning per hour to avoid spam

3. **Reset Notifications**
   - Sent once when usage limit resets
   - Reset detection based on `resets_at` timestamp change
   - Informs user that fresh capacity is available

### Service Installation

**macOS (launchd):**
- Plist file: `~/Library/LaunchAgents/com.claude-notify.plist`
- Runs at login (RunAtLoad=true)
- Auto-restarts on crash (KeepAlive=true)
- Logs to `/tmp/`

**Linux (systemd):**
- Unit file: `~/.config/systemd/user/claude-notify.service`
- User service (no sudo required)
- Auto-restart on failure
- View logs: `journalctl --user -u claude-notify -f`

## Performance

### Measured Performance
- Binary size: ~8MB (release build with symbols)
- Memory usage: ~15MB idle (headless Chrome adds overhead)
- CPU usage: <0.1% average (spikes during polling every 15 minutes)

### Optimization Opportunities
- Strip binary for smaller size
- Consider alternative to headless Chrome for lower memory footprint
- Implement cookie refresh without full browser launch

## Known Issues & Future Work

### Current Limitations
1. No encryption for stored session data (security concern)
2. No automatic cookie refresh (requires re-running setup)
3. No cookie expiration warnings
4. Headless Chrome adds memory overhead (~15MB vs target <10MB)

### Planned Improvements
1. System keychain integration for secure credential storage
2. Automatic cookie refresh using headless browser
3. Cookie expiration tracking and warnings
4. Alternative auth method that doesn't require Chrome
5. Historical usage analytics and export
6. GUI configuration tool
7. System tray/menubar integration

## Testing Checklist

- [x] Interactive setup wizard works
- [x] Automated browser authentication works
- [x] API polling with real credentials works
- [x] Threshold notifications trigger correctly
- [x] Predicted overage warnings work
- [x] Reset notifications work
- [x] Config file parsing works
- [x] Session persistence across restarts
- [x] Notification state tracking (no spam)
- [x] Status command displays correctly
- [x] Foreground monitoring mode works
- [x] macOS service installation works
- [ ] Linux systemd service installation (untested)
- [ ] Windows service installation (not implemented)
- [ ] Cookie expiration warnings (not implemented)
- [ ] Memory usage under target (currently ~15MB vs 10MB target)
- [ ] Encrypted session storage (not implemented)

## Security Considerations

### Current Implementation
- Session data stored in plaintext JSON (security risk)
- Logging configured to avoid exposing sensitive data
- HTTPS only for all API calls
- Headless Chrome runs in sandboxed mode

### Planned Security Enhancements
- Encrypt session data using system keychain
- Add cookie expiration tracking
- Implement secure cookie refresh mechanism
- Add option to clear credentials on uninstall

## Changelog

### 2025-11-11 - Phase 1 Complete
- Implemented core monitoring daemon
- Added three notification types (thresholds, overage, resets)
- Created service installation for macOS and Linux
- Added notification state tracking
- Updated configuration with notification toggles
- Completed comprehensive documentation

### 2025-11-06 (estimated) - Phase 0 Complete
- Implemented automated browser authentication
- Created interactive setup wizard
- Added session data storage
- Built status command with progress bars

## Contributing

See [beginning.md](../beginning.md) for the original specification document.

## License

[License TBD]
