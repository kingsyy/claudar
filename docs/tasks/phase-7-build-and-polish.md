# Phase 7 — Cross-Platform Build + Icons + Polish

**Depends on:** Phase 6 (all screens implemented and working)

## Goal

Produce an installable, distributable app. Tray icon colour variants are wired up to real assets. App icons are set. `tauri build` produces the correct installer for each target platform. Linux and Windows build paths are documented. The app is ready to hand to a user to install.

## In scope

- **Tray icon assets**: 5 variants (green, yellow, orange, red, grey) for macOS (22×22 px template images or coloured), Linux (SVG or PNG), Windows (16×16 ICO). `TrayIcon::set_icon()` at runtime to swap between them.
- **App icon**: a single source icon at ≥1024×1024, converted to all required sizes via `tauri icon` CLI command; placed in `src-tauri/icons/`
- **`tauri.conf.json`** build targets configured for:
  - macOS: `.app` bundle + `.dmg` installer
  - Linux: `.AppImage` + `.deb`
  - Windows: `.exe` + `.msi`
- **macOS build verified locally**: `cargo tauri build` produces a `.dmg` that installs and runs correctly on the developer's machine
- **Linux/Windows build documented**: CI matrix or manual cross-compile steps written up in `docs/building.md` (or `CONTRIBUTING.md`); not required to run in CI this phase
- **Window behaviour polish**: confirm window remembers its last position and size across launches (Tauri window state plugin, or `tauri.conf.json` `saveWindowState` if supported)
- **App name and bundle identifier** set correctly in `tauri.conf.json` (`claude-notify`, reverse-DNS bundle ID)
- **`cargo tauri build` with `--release`** produces a working binary with no debug assertions

## Out of scope

- Code signing or notarization for distribution (requires Apple Developer account)
- GitHub Actions CI pipeline (beyond the documentation of steps)
- In-app auto-update mechanism (parked in design docs)
- Windows Chrome detection wizard (parked)
- Dark/light mode theming (parked)
- Keychain integration (parked)

## Data flow

No new data flows. This phase is build infrastructure and asset wiring.

## Acceptance criteria

- [ ] `cargo tauri build` completes without errors on macOS
- [ ] The resulting `.dmg` installs and the app opens, monitors, and notifies correctly end-to-end
- [ ] Tray icon displays the correct colour based on current usage level (all 5 states reachable)
- [ ] App icon appears correctly in the Dock (macOS) and in the installed `.app` bundle
- [ ] Window position and size are preserved across relaunches
- [ ] `docs/building.md` documents the steps to produce Linux `.AppImage`/`.deb` and Windows `.exe`/`.msi` builds (even if not verified in CI)
- [ ] No debug-only code paths or `unwrap()` panics reachable in normal operation (review `--release` run for any panics)

## Design references

- See `02-architecture.md` — "Components", "Cargo Workspace Layout"
- See `03-decisions.md` — "UI Framework: Tauri 2.0" (cross-platform requirement), "Tray Icon: Status colour encoding"
- See `01-landscape.md` — "Tauri 2.0 Key Capabilities" (build section), "System Tray Behavior by Platform"
