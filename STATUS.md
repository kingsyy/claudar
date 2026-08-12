---
intent: share
stage: complete
share_target: maintained
next: Ship the tray main-thread crash fix (branch worktree-fix-tray-main-thread) as 0.4.5 and confirm the tray icon/menu still update live; then runtime-verify the web dashboard (bind/restart-on-config-change/agent API gating) and notifications/login/start-minimized on macOS/Windows; code-sign/notarize for distribution
blocker: null
updated: 2026-08-13
---

# Claudar — Monitor Claude.ai usage limits with native desktop notifications

## What it is

A lightweight Rust daemon that fetches real-time Claude.ai usage data (bypassing local-only tracking) and sends native desktop notifications when approaching rate limits. Supports multiple accounts, configurable thresholds, and runs as a background service on macOS and Linux.

## Current state

**GUI app is functionally complete.** All 7 original build phases are done (Phase 1: workspace refactor, Phase 2: Tauri skeleton, Phase 3: monitor loop integration, Phase 4: Dashboard screen, Phase 5: wizard, Phase 6: History/Accounts/Settings, Phase 7: icons/builds). The Tauri app bundles the monitor loop and runs it as a background tokio task. Screens include Dashboard (5h/7d gauges, reset countdowns, predicted burn), History (SVG chart, 7-day stats), Accounts (multi-account list, add/remove with wizard re-use), Settings (poll interval, thresholds, notifications toggle, autostart). Wizard guides new users through login (in-app webview auth), threshold setup, and autostart toggle.

**Post-phase review completed** — agent-loop run found 2 bugs (fixed during review), locked in 6 UX decisions (CLI deprecation, instance naming, welcome copy, dashboard UX, simultaneous CLI/GUI warning), and identified 1 remaining task: **Phase 8 — threshold list editing.** Settings currently only exposes the 4th (highest) of 4 hardcoded thresholds per limit, which creates non-monotonic configs and surprise notifications from hidden thresholds. Phase 8 will add a proper editable list UI: users can add/delete 1–5 thresholds per limit, kept sorted.

`cargo tauri build` produces a working `.app` locally (verified by launching). Code-signing/notarization (for distribution outside the dev machine) is out of scope for this phase but required before real release.

**v0.4.1 (2026-06-22):** Settings/UX pass — start-minimized-at-login (gated to login launches via a `--minimized` autostart flag, with a macOS dock-Reopen handler and off-screen-window recentering), accessible On/Off toggles (text + ✓/✕ + colour + position, colourblind/RTL-safe), poll-interval changes now preserve history retention in days with a confirming message, and an overuse warning below 5-minute intervals. Plus the About panel (version/build/license/source). Built and shipped as `Claudar_0.4.1_aarch64.dmg`; start-minimized + dock-Reopen not yet runtime-verified on a real login.

**v0.4.0 (2026-06-22):** cross-platform hardening pass — rewrote browser detection (machine-wide + per-user locations for Chrome/Edge/Brave/Chromium with a `$PATH` fallback), unified the notifier onto `tauri-plugin-notification` for all OSes, plus a Dashboard header redesign. `cargo check`/core tests green; GUI notification + login paths on real macOS/Windows still need a manual runtime check.

**Tray crash fixed (2026-08-13, unreleased):** v0.4.4 crashed after 6 days up (SIGTRAP, `claudar-app-2026-08-13-011938.ips`). The poll task was mutating the tray from a tokio worker, racing the non-atomic `Rc` refcount inside `tray_icon::TrayIcon` that Tauri only marks `Send` on the promise it stays on the main thread; a lost update drove it to zero and the status item was torn down off-thread. `set_tray_icon`/`refresh_tray_menu` now dispatch via `run_on_main_thread`. See DEVLOG 2026-08-13. Builds and tests clean; live tray behaviour still wants a manual check.

## Next

- **Option A:** Ship the tray fix as 0.4.5 and confirm the icon/menu still update on the running app.
- **Option B:** Code-sign/notarize the macOS `.app` for testable distribution, then run Linux/Windows builds on actual hardware (`docs/building.md` has the steps).
- **Option C:** Test the current build on real hardware as-is (no code-signing, local distribution only).
