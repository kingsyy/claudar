---
intent: share
stage: prototype
share_target: maintained
next: Phase 6 — History, Accounts, and Settings screens
blocker: null
updated: 2026-06-07
---

# Claude Notify — Monitor Claude.ai usage limits with native desktop notifications

## What it is

A lightweight Rust daemon that fetches real-time Claude.ai usage data (bypassing local-only tracking) and sends native desktop notifications when approaching rate limits. Supports multiple accounts, configurable thresholds, and runs as a background service on macOS and Linux.

## Current state

**Mid-rebuild: turning the CLI into a Tauri/Svelte desktop GUI.** The original Rust CLI daemon (monitoring loop, Chrome-based auth, notifications, service install, multi-account support) was refactored into a reusable `claude-notify-core` library (Phase 1), then wrapped in a Tauri 2.0 app with a Svelte 5 + Tailwind/shadcn-svelte frontend (Phase 2 — tray icon, window, sidebar nav scaffold). Phase 3 embedded the monitor loop directly in the Tauri process (background polling tasks, `usage-update`/`auth-required`/`monitor-error` events, tray icon colour, IPC commands). Phase 4 (just completed today) wires the Svelte Dashboard up to all of that: it subscribes to `usage-update`/`monitor-error`, fetches initial state via `get_usage`, and renders a per-instance tab switcher, live 5h/7d circular gauges, reset countdowns (ticking every 30s), a predicted burn-rate stat, and loading/error states.

**Phase 5 (just completed)** added the first-run onboarding wizard: a four-step flow (Welcome → in-app Claude.ai login via an embedded Tauri `WebviewWindow` → threshold configuration → done/open-at-login). The new `start_auth` Tauri command opens the login webview, watches for the post-login redirect, extracts session cookies straight from the webview's cookie store, resolves the org id via a new `fetch_org_id` helper, and saves `sessions/{instance}.json` — no Chrome process required. `set_autostart`/`get_autostart` wire the Done step's toggle to `tauri-plugin-autostart`. The same `start_auth` flow now also powers a non-blocking re-auth banner shown when the monitor loop emits `auth-required`.

**Next up — Phase 6 (History, Accounts, Settings screens)**: wire up the remaining stub routes — time-series history chart, multi-account management (reusing the wizard's auth step for "Add account"), and settings (polling interval, thresholds, notifications, autostart). Full build plan in `docs/todo.md` (phase 7: cross-platform packaging + icons + polish).

## Next

- Phase 5 — Onboarding wizard (Welcome / account setup / thresholds / done steps, in-app `start_auth` Chrome window flow) — see `docs/tasks/phase-5-onboarding-wizard.md`
