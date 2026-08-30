# Claudar — GUI

A Tauri 2.0 desktop app that wraps the existing `claudar` daemon with a full GUI: onboarding wizard, live usage dashboard, history chart, account management, and settings — plus a system tray icon for at-a-glance status. The existing CLI and daemon remain intact for power users.

## Table of Contents

| Doc | Description |
|-----|-------------|
| [01-landscape.md](./01-landscape.md) | Framework options, charting libraries, tray platform caveats, daemon architecture options |
| [02-architecture.md](./02-architecture.md) | Component map, process model, screen flow, data flow, IPC surface, Cargo workspace layout |
| [03-decisions.md](./03-decisions.md) | All key decisions with rationale and alternatives considered |
| [04-multi-provider.md](./04-multi-provider.md) | Research + decisions for monitoring OpenAI/Codex and other providers alongside Claude |
| [references.md](./references.md) | Every URL consulted, grouped by topic |

---

## MVP Scope

### In

| Feature | Notes |
|---------|-------|
| Onboarding wizard | Multi-step: welcome → Chrome auth → thresholds → done. No terminal needed. |
| System tray icon | Colour-coded by usage level (green/yellow/orange/red). Left-click toggles window. |
| Dashboard | Live 5h + 7d gauges, reset countdowns, per-instance tab switcher |
| History view | Area chart of 5h usage over last 7 days, with window summaries |
| Accounts screen | List instances, add (triggers wizard), remove |
| Settings screen | Polling interval, thresholds, notifications on/off, open-at-login toggle |
| Open at login | Via `tauri-plugin-autostart` — macOS LaunchAgent, Linux autostart, Windows registry |
| Cross-platform build | macOS `.app`/`.dmg`, Linux `.AppImage`/`.deb`, Windows `.exe`/`.msi` |

### Parked

| Feature | Revisit when |
|---------|-------------|
| Keychain / secure session storage | Security audit or user demand |
| Windows Chrome detection wizard | Targeting Windows seriously |
| In-app auto-update | Preparing a public release channel |
| Dark/light theme toggle | shadcn-svelte theming stabilises |
| Multiple history instances side-by-side | Multi-account is common in practice |

---

## Stack

```
Tauri 2.0
  Rust backend: claudar-core (existing modules, extracted as lib crate)
  Frontend: Svelte 5 + Vite + shadcn-svelte + LayerChart
  Tray: TrayIconBuilder (Rust)
  Autostart: tauri-plugin-autostart
  Build: tauri build → platform-native installers
```
