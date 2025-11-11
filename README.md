# Claude Notify

A lightweight Rust daemon that monitors Claude Code usage and sends native macOS notifications when approaching usage limits.

## Features

- **Automated Browser Login**: Headless Chrome automation handles the login flow
- **Native OS Notifications**: Cross-platform notifications (macOS, Linux, Windows) when approaching usage thresholds
- **Smart Monitoring**: Threshold crossings, predicted overage warnings, and reset notifications
- **Configurable Thresholds**: Monitor both 5-hour and 7-day usage limits
- **Background Service**: Install as launchd (macOS) or systemd (Linux) service for automatic monitoring
- **Minimal Footprint**: Low memory and CPU usage

## Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))
- Chrome or Chromium browser installed on your system

## Installation

```bash
# Clone the repository
git clone <repository-url>
cd claude-notify

# Build the project
cargo build --release

# Install to your PATH (optional)
cargo install --path .
```

## Setup

Run the interactive setup wizard:

```bash
cargo run -- setup
# or if installed:
# claude-notify setup
```

The setup wizard will:

1. **Ask for your email** - Enter the email associated with your Claude.ai account
2. **Launch a headless browser** - Automatically navigates to claude.ai/login
3. **Submit your email** - Fills in the email field and submits
4. **Prompt for verification code** - Check your email and enter the 6-digit code
5. **Complete login** - Submits the code and extracts session cookies
6. **Extract organization ID** - Automatically finds your org ID from the URL
7. **Test API connection** - Verifies everything works
8. **Save configuration** - Stores credentials securely

## Usage

### Check Current Status

```bash
# View current usage status with progress bars
claude-notify status

# View with verbose debug output
claude-notify status --verbose
```

### Monitor in Foreground

```bash
# Run monitor in foreground (shows logs, press Ctrl+C to stop)
claude-notify run

# Run with verbose output
claude-notify run --verbose
```

### Install as Background Service

```bash
# Install as system service (launchd on macOS, systemd on Linux)
claude-notify setup-service

# Start the background service
claude-notify start

# Stop the background service
claude-notify stop

# Uninstall the service
claude-notify uninstall-service
```

**Note**: After installing as a service, it will automatically start monitoring in the background. On macOS, logs are written to `/tmp/claude-notify.log` and `/tmp/claude-notify.error.log`.

## Configuration

Configuration is stored at: `~/.config/claude-notify/config.toml`

Default settings:
```toml
[general]
poll_interval_seconds = 900  # 15 minutes (how often to check usage)

[thresholds]
five_hour = [50, 70, 90]  # Percentage thresholds for 5-hour limit
seven_day = [50, 70, 90]  # Percentage thresholds for 7-day limit

[notifications]
sound = true                          # Enable notification sounds
persistent = false                    # Keep notifications on screen
notify_threshold_crossings = true    # Alert when crossing thresholds (50%, 70%, 90%)
notify_predicted_overage = true      # Warn if predicted to exceed 100% before reset
notify_resets = true                 # Notify when usage limits reset
```

You can customize these settings by editing the config file directly. Changes take effect on the next monitoring cycle.

Session data (cookies) is stored at: `~/.config/claude-notify/session.json`
Monitor state (notification tracking) is stored at: `~/.config/claude-notify/monitor_state.json`

## How It Works

1. **Headless Browser Authentication**: Uses `headless_chrome` to automate the login flow, bypassing Cloudflare's bot detection
2. **Cookie Extraction**: Extracts all necessary cookies including `sessionKey`, `cf_clearance`, and Cloudflare bot management tokens
3. **API Polling**: Makes authenticated requests to `claude.ai/api/organizations/{org_id}/usage`
4. **Smart Notifications**: Calculates usage rate and sends timely notifications before hitting limits

### Notification Types

The monitor sends three types of notifications (all configurable via config.toml):

1. **Threshold Crossings** (50%, 70%, 90%)
   - Notified once per threshold per reset period
   - Urgency level increases with threshold (Low → Normal → Critical)
   - Example: "You've used 70% of your 5-hour limit. Resets in 2h 15m"

2. **Predicted Overage** (5-hour limit only)
   - Analyzes current usage rate vs. time elapsed
   - Warns if predicted to exceed 100% before reset
   - Only warns once per hour to avoid spam
   - Example: "At current pace, you'll use 115% of your 5-hour limit before reset"

3. **Reset Notifications**
   - Sent when usage limits reset
   - Lets you know fresh capacity is available
   - Example: "Your 5-hour usage limit has been reset. You now have fresh capacity available"

The monitor maintains state to avoid duplicate notifications and automatically clears notification state when limits reset.

## Project Status

**Phase 0: Setup Wizard** ✅ Complete
- Automated browser login
- Email verification code flow
- Cookie extraction
- Organization ID detection

**Phase 1: Core Monitoring** ✅ Complete
- API polling with configurable intervals
- Threshold crossing detection
- Predicted overage warnings (5-hour limit)
- Reset notifications
- Native OS notifications (macOS/Linux/Windows via notify-rust)
- Status command with progress bars
- Foreground monitoring mode
- Background service installation (launchd/systemd)
- Smart notification state tracking (no spam)

**Phase 2: Polish** 📋 Planned
- Auto cookie refresh
- GUI configuration tool
- Advanced usage analytics

## Troubleshooting

### "Failed to launch browser"
- Make sure Chrome or Chromium is installed
- Check that Chrome is in your PATH
- Try running with `RUST_LOG=debug` for more details

### "API test failed"
- Verify your email is correct
- Check that you entered the verification code correctly
- Ensure you have an active Claude.ai account

### Cookies expire quickly
- The tool will need periodic re-authentication
- Future versions will handle automatic cookie refresh

## Development

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- setup

# Run tests (coming soon)
cargo test

# Build optimized release
cargo build --release
strip target/release/claude-notify
```

## License

[License TBD]

## Contributing

Contributions welcome! Please open an issue or PR.
