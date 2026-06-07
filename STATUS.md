---
intent: share
stage: prototype
share_target: maintained
next: Phase 5 — Onboarding Wizard (Welcome / account setup / thresholds / done steps)
blocker: null
updated: 2026-06-07
---

# Claude Notify — Monitor Claude.ai usage limits with native desktop notifications

## What it is

A lightweight Rust daemon that fetches real-time Claude.ai usage data (bypassing local-only tracking) and sends native desktop notifications when approaching rate limits. Supports multiple accounts, configurable thresholds, and runs as a background service on macOS and Linux.

## Current state

**Mid-rebuild: turning the CLI into a Tauri/Svelte desktop GUI.** The original Rust CLI daemon (monitoring loop, Chrome-based auth, notifications, service install, multi-account support) was refactored into a reusable `claude-notify-core` library (Phase 1), then wrapped in a Tauri 2.0 app with a Svelte 5 + Tailwind/shadcn-svelte frontend (Phase 2 — tray icon, window, sidebar nav scaffold). Phase 3 embedded the monitor loop directly in the Tauri process (background polling tasks, `usage-update`/`auth-required`/`monitor-error` events, tray icon colour, IPC commands). Phase 4 (just completed today) wires the Svelte Dashboard up to all of that: it subscribes to `usage-update`/`monitor-error`, fetches initial state via `get_usage`, and renders a per-instance tab switcher, live 5h/7d circular gauges, reset countdowns (ticking every 30s), a predicted burn-rate stat, and loading/error states.

**Next up — Phase 5 (Onboarding wizard)**: build the first-run flow (Welcome → account setup with the in-app Chrome auth window → threshold configuration → done/open-at-login prompt) so a fresh install can get from zero to a working monitored account. Full build plan in `docs/todo.md` / `docs/phases.md` (phases 6-7: history/accounts/settings screens, cross-platform packaging).

## Next

- Phase 5 — Onboarding wizard (Welcome / account setup / thresholds / done steps, in-app `start_auth` Chrome window flow) — see `docs/tasks/phase-5-onboarding-wizard.md`
