---
intent: share
stage: complete
share_target: maintained
next: Code-sign/notarize for distribution; or Phase 8 (threshold list editing UI)
blocker: null
updated: 2026-06-10
---

# Claudar — Monitor Claude.ai usage limits with native desktop notifications

## What it is

A lightweight Rust daemon that fetches real-time Claude.ai usage data (bypassing local-only tracking) and sends native desktop notifications when approaching rate limits. Supports multiple accounts, configurable thresholds, and runs as a background service on macOS and Linux.

## Current state

**GUI app is functionally complete.** All 7 original build phases are done (Phase 1: workspace refactor, Phase 2: Tauri skeleton, Phase 3: monitor loop integration, Phase 4: Dashboard screen, Phase 5: wizard, Phase 6: History/Accounts/Settings, Phase 7: icons/builds). The Tauri app bundles the monitor loop and runs it as a background tokio task. Screens include Dashboard (5h/7d gauges, reset countdowns, predicted burn), History (SVG chart, 7-day stats), Accounts (multi-account list, add/remove with wizard re-use), Settings (poll interval, thresholds, notifications toggle, autostart). Wizard guides new users through login (in-app webview auth), threshold setup, and autostart toggle.

**Post-phase review completed** — agent-loop run found 2 bugs (fixed during review), locked in 6 UX decisions (CLI deprecation, instance naming, welcome copy, dashboard UX, simultaneous CLI/GUI warning), and identified 1 remaining task: **Phase 8 — threshold list editing.** Settings currently only exposes the 4th (highest) of 4 hardcoded thresholds per limit, which creates non-monotonic configs and surprise notifications from hidden thresholds. Phase 8 will add a proper editable list UI: users can add/delete 1–5 thresholds per limit, kept sorted.

`cargo tauri build` produces a working `.app` locally (verified by launching). Code-signing/notarization (for distribution outside the dev machine) is out of scope for this phase but required before real release.

## Next

- **Option A:** Run Phase 8 (threshold list editing UI) — well-defined, moderate scope, improves the Settings experience.
- **Option B:** Code-sign/notarize the macOS `.app` for testable distribution, then run Linux/Windows builds on actual hardware (`docs/building.md` has the steps).
- **Option C:** Test the current build on real hardware as-is (no code-signing, local distribution only).
