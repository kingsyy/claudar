# Building Claudar

The GUI app is a Tauri 2.0 application (`src-tauri/`) with a Svelte frontend (`ui/`). Producing
an installable build means running `cargo tauri build`, which compiles the Rust binary, bundles
the frontend, and packages everything into the platform-native installer formats configured in
`src-tauri/tauri.conf.json`.

## Prerequisites (all platforms)

- Rust toolchain (`rustup`) and `cargo`
- Node.js + npm (for the `ui/` frontend build)
- The Tauri CLI: `cargo install tauri-cli --version "^2"`

Install frontend dependencies once with `npm install --prefix ui`.

## macOS

Targets: `.app` bundle + `.dmg` installer (configured via `bundle.targets: ["app", "dmg", ...]`).

```bash
cargo tauri build
```

This produces, under `src-tauri/target/release/bundle/`:

- `macos/Claudar.app` — the application bundle
- `dmg/Claudar_<version>_<arch>.dmg` — the disk-image installer

Open the `.dmg`, drag the app into `/Applications`, and launch it. No code signing or
notarization is configured (out of scope for this phase) — Gatekeeper will warn on first launch;
right-click → Open to bypass, or sign/notarize separately for distribution outside your own
machine.

> **Automation permission for DMG creation**: `bundle_dmg.sh` drives Finder via AppleScript to
> lay out the install window (background, icon positions, `Applications` symlink). If the
> terminal/shell running `cargo tauri build` has not been granted **Automation** access to
> Finder (System Settings → Privacy & Security → Automation), the script fails with
> `Finder got an error: AppleEvent timed out (-1712)` after the `.app` bundle has already been
> produced successfully. Grant the permission once (macOS will prompt on first run from a
> Terminal with UI access) and re-run `cargo tauri build` to produce the `.dmg`.

## Linux

Targets: `.AppImage` + `.deb` (configured via `bundle.targets`).

Build on a Linux machine (or in a Linux container/VM — Tauri does not support cross-bundling
Linux artifacts from macOS):

```bash
# Debian/Ubuntu build dependencies
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev

cargo install tauri-cli --version "^2"
npm install --prefix ui
cargo tauri build
```

Output, under `src-tauri/target/release/bundle/`:

- `appimage/claudar_<version>_<arch>.AppImage` — portable, run directly with `chmod +x` then execute
- `deb/claudar_<version>_<arch>.deb` — install with `sudo dpkg -i <file>.deb`

### CI matrix sketch

A GitHub Actions job for Linux artifacts would look like:

```yaml
runs-on: ubuntu-22.04
steps:
  - uses: actions/checkout@v4
  - uses: dtolnay/rust-toolchain@stable
  - uses: actions/setup-node@v4
    with: { node-version: 20 }
  - run: sudo apt update && sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  - run: npm install --prefix ui
  - run: cargo install tauri-cli --version "^2"
  - run: cargo tauri build
  - uses: actions/upload-artifact@v4
    with:
      name: linux-bundles
      path: src-tauri/target/release/bundle/{appimage,deb}/*
```

## Windows

Targets: `.exe` (NSIS installer) + `.msi` (configured via `bundle.targets: ["msi", "nsis"]`).

Build on a Windows machine (native or via a `windows-latest` GitHub Actions runner — Tauri does
not support cross-bundling Windows artifacts from macOS/Linux):

```powershell
# Requires the Microsoft C++ Build Tools (Visual Studio Build Tools, "Desktop development with C++")
# and the WebView2 runtime (preinstalled on modern Windows; otherwise download from Microsoft)

cargo install tauri-cli --version "^2"
npm install --prefix ui
cargo tauri build
```

Output, under `src-tauri\target\release\bundle\`:

- `nsis\Claudar_<version>_<arch>-setup.exe` — NSIS installer (`.exe`)
- `msi\Claudar_<version>_<arch>.msi` — MSI installer

### CI matrix sketch

```yaml
runs-on: windows-latest
steps:
  - uses: actions/checkout@v4
  - uses: dtolnay/rust-toolchain@stable
  - uses: actions/setup-node@v4
    with: { node-version: 20 }
  - run: npm install --prefix ui
  - run: cargo install tauri-cli --version "^2"
  - run: cargo tauri build
  - uses: actions/upload-artifact@v4
    with:
      name: windows-bundles
      path: src-tauri/target/release/bundle/{nsis,msi}/*
```

## Notes

- `cargo tauri build` always builds in `--release` mode by default; debug assertions and
  `dev`-only behaviour (e.g. devtools) are compiled out.
- Code signing/notarization (macOS) and Authenticode signing (Windows) are not configured —
  see the Phase 7 task notes for why this is deliberately out of scope for now.
- An in-app auto-update mechanism is parked; cross-platform release artifacts are produced
  manually for now.
