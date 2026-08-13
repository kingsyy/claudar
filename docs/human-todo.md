# Human to-do — Claudar GUI

Tasks only you can do — external setup, credentials, platform config, decisions. agent-loop
cannot do these. Clear the blocking ones before running the phase they gate.

> **Ticked 2026-08-10 from direct local evidence, not from memory.** The evidence is named on
> every new tick. An unticked box is not proof of undone work: it may be genuinely open or not
> verifiable from this machine, so check before repeating one.

## Before starting

- [x] Install Tauri CLI: `cargo install tauri-cli --version "^2.0"` — done, `cargo tauri --version` returned `tauri-cli 2.11.2`, verified 2026-08-10.
- [x] Install Node.js ≥ 20 and npm/pnpm (required for the Svelte frontend build) — done, `node`, `npm`, and `pnpm` are on PATH, verified 2026-08-10.
- [x] Confirm `cargo build` still passes on the existing codebase before handing off to Phase 1 — done, `cargo build` exited 0, verified 2026-08-10.

## Before Phase 2 — Tauri App Skeleton

- [x] Run `create-tauri-app` or copy the `alysonhower/tauri2-svelte5-shadcn` template as the starting point for `src-tauri/` and `ui/` — agent-loop will adapt it, but the scaffold must exist before it begins. — done, `src-tauri/Cargo.toml` and `ui/package.json` exist, verified 2026-08-10.
- [ ] Decide: do you want the main window to open at a fixed size or resizable? (Default: resizable with a minimum of ~800×600)

## Before Phase 7 — Cross-Platform Build + Icons + Polish

- [ ] Provide a source app icon (PNG or SVG, ≥1024×1024). The `tauri icon` command generates all platform sizes from this one file.
- [ ] Provide tray icon designs — or confirm: agent-loop generates plain filled-colour SVGs (green/yellow/orange/red/grey circles with a small "C" or usage indicator). Just say the word.
- [ ] macOS code signing: for personal use, ad-hoc signing is fine (no account needed). For distribution, you'll need an Apple Developer account.
- [ ] Decide: GitHub Actions CI matrix for Linux/Windows builds, or document manual steps only? (Default: document only)

## Decisions you'll need to make

- [ ] **CLI `setup` command**: with the new GUI, the old `claudar setup` command (which launched a visible Chrome window) is no longer needed by GUI users. Options: (a) remove it from the CLI, (b) keep it but mark it deprecated, (c) keep it working as-is. Which do you prefer?
- [ ] **Instance name for the default account**: the CLI uses `"default"` internally — confirm this carries forward as the display name in the wizard, or choose something friendlier (e.g. "My Claude account").
- [ ] **Simultaneous CLI + GUI use**: running both the CLI daemon and the GUI app would double-notify. Do you want the GUI to detect and warn the user if a launchd service is already running, or just document it?
- [ ] **Welcome copy**: the Step 1 headline and explanation in the wizard are placeholder language in the task file. If you have a preferred tone or specific wording, share it before Phase 5 runs — otherwise agent-loop will write reasonable copy and you can edit it after.
