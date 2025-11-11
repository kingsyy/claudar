# Claude Notify

A lightweight Rust daemon that monitors Claude Code usage and sends native macOS notifications when approaching usage limits.

## Features

- **Automated Browser Login**: Headless Chrome automation handles the login flow
- **Native Notifications**: macOS notifications when approaching usage thresholds
- **Configurable Thresholds**: Monitor both 5-hour and 7-day usage limits
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

```bash
# Check current usage status (coming soon)
claude-notify status

# Run the monitoring daemon (coming soon)
claude-notify run

# Install as a service (coming soon)
claude-notify install-service
```

## Configuration

Configuration is stored at: `~/.config/claude-notify/config.toml`

Default settings:
```toml
[general]
poll_interval_seconds = 900  # 15 minutes

[thresholds]
five_hour = [50, 70, 90]
seven_day = [50, 70, 90]

[notifications]
sound = true
persistent = false
```

Session data (cookies) is stored at: `~/.config/claude-notify/session.json`

## How It Works

1. **Headless Browser Authentication**: Uses `headless_chrome` to automate the login flow, bypassing Cloudflare's bot detection
2. **Cookie Extraction**: Extracts all necessary cookies including `sessionKey`, `cf_clearance`, and Cloudflare bot management tokens
3. **API Polling**: Makes authenticated requests to `claude.ai/api/organizations/{org_id}/usage`
4. **Smart Notifications**: Calculates usage rate and sends timely notifications before hitting limits

## Project Status

**Phase 0: Setup Wizard** ✅ Complete
- Automated browser login
- Email verification code flow
- Cookie extraction
- Organization ID detection

**Phase 1: Core Monitoring** 🚧 In Progress
- API polling
- Threshold calculations
- Native notifications
- Status command

**Phase 2: Polish** 📋 Planned
- Service installation
- Rate-based predictions
- Auto cookie refresh

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
