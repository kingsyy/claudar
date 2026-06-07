---
intent: share
stage: prototype
share_target: maintained
next: Phase 4 — Svelte Dashboard Screen (subscribe to usage-update events, render live gauges)
blocker: null
updated: 2026-06-07
---

# Claude Notify — Monitor Claude.ai usage limits with native desktop notifications

## What it is

A lightweight Rust daemon that fetches real-time Claude.ai usage data (bypassing local-only tracking) and sends native desktop notifications when approaching rate limits. Supports multiple accounts, configurable thresholds, and runs as a background service on macOS and Linux.

## Current state

**Mid-rebuild: turning the CLI into a Tauri/Svelte desktop GUI.** The original Rust CLI daemon (monitoring loop, Chrome-based auth, notifications, service install, multi-account support) was refactored into a reusable `claude-notify-core` library (Phase 1), then wrapped in a Tauri 2.0 app with a Svelte 5 + Tailwind/shadcn-svelte frontend (Phase 2 — tray icon, window, sidebar nav scaffold). Phase 3 (just completed today) embeds the monitor loop directly in the Tauri process: it spawns one polling task per instance, emits `usage-update`/`auth-required`/`monitor-error` events to the frontend, drives the tray icon colour, and exposes `get_instances`/`get_config`/`set_config`/`get_usage` IPC commands.

**Next up — Phase 4 (Svelte Dashboard screen)**: wire the frontend to subscribe to `usage-update` events and render live 5h/7d usage gauges, reset countdowns, a per-instance tab switcher, and loading/error states. This is the first genuinely usable screen — after it the app works as a monitoring tool end-to-end. Full build plan in `docs/todo.md` / `docs/phases.md` (phases 5-7: onboarding wizard, history/accounts/settings screens, cross-platform packaging).

## Next

- Phase 4 — Svelte Dashboard screen (subscribe to `usage-update`, render live 5h/7d gauges, reset countdown, per-instance tabs, loading/error states) — see `docs/tasks/phase-4-dashboard.md`
