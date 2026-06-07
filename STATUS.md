---
intent: share
stage: in-progress
share_target: maintained
next: Phase 4 — Svelte Dashboard Screen (subscribe to usage-update events, render live gauges)
blocker: null
updated: 2026-06-07
---

# Claude Notify — Monitor Claude.ai usage limits with native desktop notifications

## What it is

A lightweight Rust daemon that fetches real-time Claude.ai usage data (bypassing local-only tracking) and sends native desktop notifications when approaching rate limits. Supports multiple accounts, configurable thresholds, and runs as a background service on macOS and Linux.

## Current state

**Works**: Core monitoring loop, browser-based authentication with Chrome, usage API polling, native notifications (macOS/Linux), service installation (launchd/systemd), multi-account support, configurable thresholds, CLI with setup/config/status/usage/run commands. Released on GitHub with pre-built binaries for macOS, Linux, and Windows. Has tests throughout codebase (time_format, config, storage, state, notifications, retry, etc.). Last commit 5 days ago (performance optimization for direct HTTP fetch in usage command).

**Minor rough edges**: Session cookies stored in plaintext (keychain integration noted as future work). Windows binary exists but platform not fully documented in README. OBSCURA_RESEARCH.md and history tracking features appear in-progress (history.rs, history_cmd.rs untracked).

## Next

- Monitor GitHub issues and user feedback for stability blockers
- Finalize and document Windows support if targeting all three platforms
- Plan keychain integration for secure session storage
