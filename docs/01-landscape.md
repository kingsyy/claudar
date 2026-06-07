# 01 — Landscape

Options researched for building a GUI on top of the existing Rust daemon.

---

## UI Framework Options

| Framework | Language | Binary size | Platforms | Native feel | Rust reuse |
|-----------|----------|-------------|-----------|-------------|-----------|
| **Tauri 2.0** | Rust + web | ~5–15 MB | macOS, Linux, Windows | Webview (OS-native) | Full — existing crates become library |
| Egui / eframe | Pure Rust | ~10–20 MB | macOS, Linux, Windows | OpenGL/Metal/wgpu | Full | 
| Swift / SwiftUI | Swift | N/A | macOS only | Native | FFI or rewrite |
| Electron | JS + Node | ~120 MB | macOS, Linux, Windows | Chromium (bundled) | Rust via node-addon-api |

**Tauri 2.0** is the clear winner: native OS webview (not bundled Chromium), strong Rust backend integration, active ecosystem, official cross-platform CI templates.

---

## Tauri 2.0 Key Capabilities

| Capability | Detail |
|------------|--------|
| IPC | `#[tauri::command]` exposes Rust fns to JS; `app.emit()` pushes Rust events to frontend |
| System tray | `TrayIconBuilder` + `MenuBuilder` — cross-platform. Linux: icon + menu work, mouse events limited |
| Multi-window | `WebviewWindowBuilder` with unique labels; show/hide via `window.show()` / `window.hide()` |
| Autostart | Official `tauri-plugin-autostart` — LaunchAgent (macOS), `~/.config/autostart` (Linux), registry (Windows) |
| Permissions | Capability-based allowlist per window; blocks dangerous commands by default |
| Build | `tauri build` → `.app` + `.dmg` (macOS), `.AppImage`/`.deb` (Linux), `.exe`/`.msi` (Windows) |
| Workspace | Tauri app lives in `src-tauri/`; existing Rust modules become a library crate in the workspace |

**Status**: Tauri 2.0 is production stable (released Oct 2024). Actively maintained. No developer-preview caveats.

---

## Frontend Framework Options (inside Tauri webview)

| Framework | Bundle size | Tauri template | Component lib | Time-series chart |
|-----------|-------------|----------------|---------------|-------------------|
| **Svelte 5 + Vite** | ~150 KB gz | `create-tauri-app` scaffold, tauri2-svelte5-shadcn | shadcn-svelte | lightweight-charts-svelte |
| React 19 + Vite | ~300 KB gz | tauri2-react-shadcn | shadcn/ui | recharts / lightweight-charts |
| Vue 3 | ~200 KB gz | official template | radix-vue | Chart.js |

**Svelte 5** chosen: smaller bundle, less boilerplate, strong Tauri template (`alysonhower/tauri2-svelte5-shadcn`), and `shadcn-svelte` added native chart support in 2026 via LayerChart.

---

## Charting (History View)

| Library | Svelte 5 | Size | Time-series | Real-time update |
|---------|----------|------|-------------|-----------------|
| **lightweight-charts-svelte** | ✅ | ~45 KB | ✅ area/line | ✅ reactive prop |
| shadcn-svelte LayerChart | ✅ | included | ✅ line | ✅ |
| Chart.js (via svelte-chartjs) | ✅ | ~160 KB | ✅ | ✅ |

Both `lightweight-charts-svelte` and shadcn-svelte's built-in LayerChart work. LayerChart preferred (zero extra dep if shadcn-svelte already in use).

---

## System Tray Behavior by Platform

| Platform | Icon location | Left-click | Right-click | Mouse events |
|----------|--------------|------------|-------------|--------------|
| macOS | Menu bar (top right) | Toggle window | Context menu | ✅ Full |
| Linux | Panel (top/bottom) | Context menu (default) | Context menu | ⚠️ Limited |
| Windows | Taskbar tray (bottom right) | Toggle window | Context menu | ✅ Full |

Linux limitation: `on_tray_icon_event` for click events not emitted. Workaround: configure left-click to always open context menu with "Show Window" item.

---

## Daemon Architecture Options

| Approach | Pros | Cons |
|----------|------|------|
| **Embedded tokio task in Tauri** | One process, tight event bus, simpler install | App must be running for monitoring |
| Separate daemon + file polling | Daemon runs even when UI is closed | Two processes, stale data risk, IPC complexity |
| Separate daemon + Unix socket | Live data when UI opens | Socket management, cross-platform complexity |

Embedded chosen: the daemon's whole purpose is to send notifications — those work fine from within the Tauri process. "Open at login" via `tauri-plugin-autostart` replaces launchd/systemd. CLI kept for debugging and power users.

---

## Workspace Structure Options

| Approach | Pros | Cons |
|----------|------|------|
| **Cargo workspace: `claude-notify-core` lib + `claude-notify` CLI bin + `claude-notify-app` Tauri bin** | Clean separation, both CLI and GUI share core logic | More upfront refactor |
| Single crate with feature flags | Less refactor | Messy, feature-flag hell |
| Separate repo | Independent versioning | Loses shared history, diverges |

Workspace approach chosen. The bulk of existing `src/` becomes `crates/claude-notify-core/src/` (a library). CLI `main.rs` and Tauri `src-tauri/main.rs` each pull from it.
