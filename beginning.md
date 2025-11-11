# Claude Code Usage Monitor - Complete Build Specification

## Project Overview
Build a lightweight Rust daemon that monitors Claude Code usage and sends native macOS notifications when approaching usage limits. Must be cross-platform ready (Mac first, Linux/Windows later).

## Core Requirements

### 1. Authentication System
- User manually provides `sessionKey` cookie from their browser (28-day expiration)
- Interactive setup wizard with clear instructions on extracting cookie from DevTools
- Store session cookie securely in local encrypted storage using system keychain
- Track cookie expiration date and warn user 3 days before it expires
- Handle cookie expiration gracefully with user notification to refresh
- User must also provide their Organization ID from Claude.ai UI

### 2. API Integration
- Usage endpoint: `https://claude.ai/api/organizations/{org_id}/usage`
- Required cookies in request: `sessionKey`, `cf_clearance`, `lastActiveOrg`
- Parse JSON response with structure:
```json
{
  "five_hour": {
    "utilization": 17.0,
    "resets_at": "2025-11-06T16:59:59.957577+00:00"
  },
  "seven_day": {
    "utilization": 15.0,
    "resets_at": "2025-11-12T07:59:59.957602+00:00"
  }
}
```
- Configurable poll interval (default: 15 minutes)
- Exponential backoff on errors
- Network failure resilience

### 3. Smart Notification Logic

#### Five-Hour Limit Notifications:
- Alert at 50%, 70%, 90% utilization
- Calculate "rate vs time" - if usage rate suggests hitting limit before reset, warn earlier

#### Seven-Day Limit Notifications:
- Compare utilization percentage vs week percentage elapsed
- Example: If 40% used but only 30% of week elapsed = warning
- Same 50%, 70%, 90% thresholds

#### Notification Rules:
- Don't spam - maximum one notification per threshold per period
- Clear, actionable messages with exact numbers
- Show time until reset
- macOS native notifications using `notify-rust` crate

### 4. Configuration
TOML config file (`~/.config/claude-code-monitor/config.toml`):
```toml
[general]
poll_interval_seconds = 900  # 15 minutes

[thresholds]
five_hour = [50, 70, 90]
seven_day = [50, 70, 90]

[notifications]
sound = true
persistent = false

[auth]
# Encrypted session storage path
session_file = "~/.config/claude-code-monitor/session.enc"
```

### 5. Technical Requirements

#### Dependencies (Cargo.toml):
- `tokio` - Async runtime
- `reqwest` - HTTP client with cookie support
- `serde` + `serde_json` - JSON parsing
- `notify-rust` - Native notifications
- `chrono` - DateTime handling
- `config` - TOML config parsing
- `ring` or `aes-gcm` - Cookie encryption
- `clap` - CLI argument parsing
- `tracing` - Logging

#### Architecture:
```
src/
├── main.rs              # Entry point, daemon loop
├── auth.rs              # Magic link → cookie exchange
├── api.rs               # Usage API client
├── calculator.rs        # Threshold & rate calculations
├── notifier.rs          # Native notification wrapper
├── config.rs            # Config management
└── storage.rs           # Encrypted session storage
```

#### Performance Goals:
- Memory footprint: < 10MB idle
- CPU usage: < 0.1% average
- Binary size: < 5MB (release build)
- Startup time: < 100ms

### 6. CLI Interface

```bash
# First time setup - interactive wizard
claude-code-monitor setup
# Prompts for:
# - Organization ID (from Claude.ai URL or UI)
# - sessionKey cookie (with instructions to copy from DevTools)
# - Optional: other cookies (cf_clearance, etc.)

# Run daemon (foreground)
claude-code-monitor run

# Run as background service
claude-code-monitor start

# Check current status
claude-code-monitor status

# Stop daemon
claude-code-monitor stop
```

### 7. macOS Service Integration
- Generate launchd plist for auto-start on login
- Install command: `claude-code-monitor install-service`
- Logs to: `~/Library/Logs/claude-code-monitor.log`

### 8. Error Handling
- Graceful degradation on network failures
- User-friendly error notifications
- Detailed logging for debugging
- Cookie expiration → prompt for new magic link

## Development Phases

### Phase 0: Setup Wizard (Critical UX)
The setup wizard should be **extremely user-friendly**. Display step-by-step instructions:

**Step 1: Get Organization ID**
```
Open Claude.ai in your browser
Click on your workspace/organization name
Look at the URL or settings - you'll see an ID like: 59b2da95-7ec0-4540-95fa-cea7c7f333c6
```

**Step 2: Get sessionKey Cookie**
```
1. Open Claude.ai in your browser (while logged in)
2. Open Developer Tools (Cmd+Option+I on Mac)
3. Go to: Application → Cookies → https://claude.ai
4. Find the cookie named "sessionKey"
5. Copy its Value (looks like: sk-ant-...)
6. Paste it here
```

The wizard should validate:
- Org ID format (UUID)
- sessionKey format (starts with expected prefix)
- Test API call before saving

### Phase 1: Core Functionality (MVP)
1. Magic link authentication
2. Basic API polling
3. Simple threshold notifications
4. Config file support

### Phase 2: Polish
5. Encrypted session storage
6. Auto cookie refresh
7. Rate-based predictions
8. launchd integration

### Phase 3: Cross-platform (Future)
9. Linux systemd support
10. Windows service support

## Testing Checklist
- [ ] Interactive setup wizard is user-friendly
- [ ] Cookie extraction instructions are clear
- [ ] API polling with real credentials works
- [ ] All notification thresholds trigger correctly
- [ ] Config file parsing works
- [ ] Cookie persistence across restarts
- [ ] Expiration warnings appear 3 days before cookie expires
- [ ] Handles network outages gracefully
- [ ] Memory usage stays under 10MB
- [ ] No CPU spikes during polling
- [ ] macOS notifications appear correctly
- [ ] launchd service installs and runs

## Security Considerations
- Never log session cookies/tokens to console or files
- Encrypt session storage using system keychain (macOS Keychain, etc.)
- Use HTTPS only for all API calls
- Validate organization ID format before use
- Cookie refresh notifications should be non-intrusive but persistent

## Nice-to-Have Features (Optional)
- Status bar icon (menubar app)
- Web dashboard for historical data
- Export usage data to CSV
- Customizable notification sounds
- Slack/Discord webhook support

## Build Instructions
```bash
# Development
cargo build

# Release (optimized)
cargo build --release
strip target/release/claude-code-monitor  # Reduce binary size

# Install
cargo install --path .
```

## Success Criteria
✅ Runs silently in background with minimal resources
✅ Accurate, timely notifications at configured thresholds
✅ Survives network issues and system restarts
✅ Simple setup process (< 2 minutes)
✅ Works seamlessly on macOS (primary target)

---

**API Endpoint to Fill In**: `[INSERT_ACTUAL_USAGE_API_URL]`

**Magic Link Auth Flow to Reverse Engineer**:
- Document what happens when magic link is clicked
- What cookies are set
- How to exchange token for session
