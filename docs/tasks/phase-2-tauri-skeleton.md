# Phase 2 — Tauri App Skeleton

**Depends on:** Phase 1 (workspace with `claudar-core` lib in place)

## Goal

Stand up the Tauri 2.0 application shell and the Svelte 5 frontend scaffold — no real functionality yet, but the app launches, a grey tray icon appears, and the window can be shown and hidden via the tray. This establishes the full project structure and build pipeline that all subsequent phases build on.

## In scope

- `src-tauri/` Tauri 2.0 application:
  - `Cargo.toml` depending on `claudar-core`
  - `main.rs` setting up the Tauri builder, registering the tray icon, and opening the main window
  - `tauri.conf.json` with window config (title, size, hidden titlebar or native as appropriate)
  - Tray icon with context menu: "Show Window" / "Hide Window" and "Quit" items
  - Left-click on tray (macOS/Windows) toggles window visibility
  - Clicking the window's close button hides it (does not quit the process)
- `ui/` Svelte 5 + Vite frontend scaffold:
  - `package.json`, `vite.config.ts` wired to Tauri
  - `shadcn-svelte` installed and initialised
  - Placeholder `App.svelte` rendering a sidebar shell with four nav items (Dashboard, History, Accounts, Settings) — content areas are empty stubs
- A single stub Tauri command (`ping` or similar) verifiable from the browser devtools console, confirming IPC is wired

## Out of scope

- Any real data or monitor logic (Phase 3)
- Implemented screens (Phases 4–6)
- Real tray icon colours or icon assets (Phase 7)
- Onboarding detection (Phase 5)

## Data flow

No real data flows yet. The tray icon is hardcoded grey. The window renders the scaffold shell with empty route stubs.

## Acceptance criteria

- [ ] `cargo tauri dev` (or equivalent) launches the app without errors
- [ ] A tray icon appears in the system tray on macOS
- [ ] Left-clicking the tray icon toggles window visibility
- [ ] Right-clicking the tray icon shows a context menu with Show/Hide and Quit items
- [ ] Closing the window via the close button hides it; the tray icon remains
- [ ] Quitting via tray context menu exits the process cleanly
- [ ] The Svelte frontend renders in the window with a sidebar containing four nav items
- [ ] IPC is confirmed working (stub command callable from browser devtools)

## Design references

- See `02-architecture.md` — "Process Model", "Screens & Navigation", "Cargo Workspace Layout"
- See `03-decisions.md` — "UI Framework: Tauri 2.0", "Frontend: Svelte 5"
