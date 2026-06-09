# Phases — Claudar GUI

| Phase | Builds | Delivers | Depends on |
|-------|--------|----------|-----------|
| **1** | Cargo workspace refactor: `claudar-core` lib + CLI bin restructured | CLI still works; core logic is a reusable library | — |
| **2** | Tauri app skeleton: `src-tauri/`, `ui/` scaffold, tray icon, window show/hide | App launches, grey tray icon visible, window toggles | Phase 1 |
| **3** | Monitor loop embedded + event bus + history writer | Real usage data, tray icon colour, native notifications — all from within the Tauri process | Phase 2 |
| **4** | Svelte Dashboard screen | Live 5h/7d gauges and reset countdown visible in the window; first usable screen | Phase 3 |
| **5** | Onboarding wizard | Zero-terminal setup: Chrome auth, thresholds, done — fully guided | Phase 4 |
| **6** | History, Accounts, and Settings screens | Complete screen set; user can manage instances and configure all settings from the UI | Phase 5 |
| **7** | Cross-platform build + tray icons + app icons | Installable `.app`/`.dmg` on macOS; documented Linux/Windows build path; coloured tray icons wired up | Phase 6 |
