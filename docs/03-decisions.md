# 03 — Decisions

---

### UI Framework: Tauri 2.0, not Electron or egui

**Decided:** Tauri 2.0

| Option | Pros | Cons |
|--------|------|------|
| **Tauri 2.0** ✅ | Rust backend reuse, OS-native webview, 5–15 MB binary, production stable, active ecosystem | Webview rendering differences across OS (mainly minor CSS) |
| Electron | Massive ecosystem, consistent rendering | 120+ MB bundle, bundled Chromium, Rust only via FFI |
| Egui | Pure Rust, no web layer | Limited component ecosystem, no polished charting, lower visual quality |
| SwiftUI | Native macOS look | macOS only, requires FFI or full rewrite |

**Why:** The project is already Rust. Tauri reuses every existing module with zero FFI. Binary stays small. Cross-platform (macOS + Linux + Windows) is required.  
**Implication:** Must use OS webview — test on all three platforms; avoid CSS that relies on Chromium-specific behaviour.

---

### Frontend: Svelte 5, not React or Vue

**Decided:** Svelte 5 + Vite + shadcn-svelte

| Option | Pros | Cons |
|--------|------|------|
| **Svelte 5** ✅ | ~150 KB bundle, minimal boilerplate, fast Vite compile, shadcn-svelte has layerchart | Smaller ecosystem than React |
| React 19 | Largest ecosystem, most tutorials | ~300 KB bundle, more boilerplate |
| Vue 3 | Good balance | Less Tauri template support in 2026 |

**Why:** For a utility desktop app, bundle size and compile speed matter more than ecosystem breadth. shadcn-svelte + LayerChart covers all component needs without additional deps.  
**Implication:** Keep frontend in `ui/` as a separate Vite project; don't use SvelteKit (static adapter workarounds aren't needed with plain Svelte + Vite).

---

### Charting: shadcn-svelte LayerChart, not a separate chart library

**Decided:** LayerChart (bundled with shadcn-svelte)

| Option | Pros | Cons |
|--------|------|------|
| **LayerChart (shadcn-svelte)** ✅ | Zero extra dep, matches design system, D3-based | Less control for financial-style charts |
| lightweight-charts-svelte | Purpose-built for time series, real-time updates | Extra dep, financial chart aesthetic |
| Chart.js | Familiar, large docs | 160 KB, bulkier |

**Why:** If shadcn-svelte is already in the stack, LayerChart adds zero weight. The history view needs a simple area/line chart over time — not a trading terminal.  
**Implication:** If richer time-series interactivity is needed later (crosshair, zoom), drop in `lightweight-charts-svelte` as an upgrade.

---

### Daemon: Embedded tokio task, not separate process

**Decided:** Monitor loop runs as a tokio task inside the Tauri process

| Option | Pros | Cons |
|--------|------|------|
| **Embedded tokio task** ✅ | One process, direct Tauri event emit, simpler install | Monitoring stops if app is quit |
| Separate launchd/systemd daemon | Runs even when UI closed | Two processes, IPC complexity, file-polling lag |

**Why:** The daemon's job is to send notifications. Those work fine from within Tauri. "Open at login" via `tauri-plugin-autostart` replaces launchd service for most users. Power users can still use the CLI daemon.  
**Implication:** CLI service commands (`install`, `uninstall`, `start`, `stop`) become deprecated for GUI users. Keep them; they still work. Document that running both simultaneously would double-notify.

---

### Workspace: Cargo workspace with shared core library

**Decided:** Extract `claudar-core` as a library crate; CLI and Tauri app both depend on it

| Option | Pros | Cons |
|--------|------|------|
| **Cargo workspace + lib crate** ✅ | Clean separation, both CLI and GUI share logic, no duplication | Upfront refactor — move `src/` to `crates/claudar-core/src/` |
| Feature flags on single crate | Less refactor | Conditional compilation complexity, hard to maintain |
| Fork/separate repo | Independent | Diverges immediately, loses shared history |

**Why:** The existing modules (`browser_auth`, `monitor`, `history`, etc.) are pure logic with no CLI dependency — they're already structured as a library. The refactor is mostly moving files and splitting `Cargo.toml`.  
**Implication:** `monitor::run_monitor()` must become `pub async fn` callable from Tauri commands. Any `eprintln!` / `println!` in core must be replaced with `tracing` events (already mostly done).

---

### Autostart: tauri-plugin-autostart, not manual launchd/systemd

**Decided:** Official `tauri-plugin-autostart`

| Option | Pros | Cons |
|--------|------|------|
| **tauri-plugin-autostart** ✅ | Cross-platform, toggle from Settings UI, no shell scripts | macOS uses LaunchAgent (same as existing service install) |
| Manual launchd plist | Already implemented in service.rs | macOS-only, no Linux/Windows parity |
| Homebrew service | Familiar to devs | GUI users don't use Homebrew |

**Why:** Gives users a toggle in the UI. Works on all three target platforms. Official Tauri plugin — maintained by the Tauri team.  
**Implication:** On macOS, test that `tauri-plugin-autostart` LaunchAgent doesn't conflict with any existing `claudar` launchd service the user may have installed via the CLI.

---

### Tray Icon: Status colour encoding

**Decided:** Tray icon changes colour based on worst-case usage level

| Level | Condition | Icon colour |
|-------|-----------|-------------|
| Green | Both windows < 50% | Green |
| Yellow | Either window 50–80% | Yellow/amber |
| Orange | Either window 80–95% | Orange |
| Red | Either window ≥ 95% | Red |
| Grey | No data / error | Grey |

**Why:** Gives instant at-a-glance status without opening the window — the primary value of a tray icon for a monitoring tool.  
**Implication:** Need 5 icon variants per platform (or a single SVG with fill colour swapped at runtime). Tauri `TrayIconBuilder::set_icon()` can be called at runtime.

---

### Onboarding: Wizard shown on first launch, not a separate CLI step

**Decided:** Multi-step wizard inside the main Tauri window on first launch (no session files found)

| Option | Pros | Cons |
|--------|------|------|
| **In-app wizard** ✅ | Discoverable, no terminal needed, guided Chrome auth | Wizard must handle Chrome window opening visibly |
| CLI `setup` command only | Already exists | Not discoverable from a GUI install |

**Why:** Users who install the GUI app will not run `claudar setup` in a terminal. The wizard must work standalone.  
**Implication:** During auth step, Chrome opens as a visible window outside the Tauri app. The wizard shows a "waiting for login…" state with a spinner. This is intentional and expected (same mechanism as current `setup` command).

---

### Auth: Tauri embedded webview, not a separate Chrome window

**Decided:** Open a Tauri `WebviewWindow` pointing at `https://claude.ai` for the login step; extract session cookies from it after login; close the window.

| Option | Pros | Cons |
|--------|------|------|
| **Tauri WebviewWindow** ✅ | No Chrome install required; login happens inside the app; cookie extraction via Tauri cookie API | OS webview (WKWebView / WebKitGTK / WebView2) must pass Cloudflare — evidence says it does |
| Visible Chrome window (current CLI) | Battle-tested with Cloudflare | Requires Chrome installed; window appears outside the app; jarring UX |
| headless Chrome | Scriptable | Requires Chrome; accumulates code-signing clones on macOS (known bug) |

**Why:** The user already confirmed that usage data fetches without issues — Cloudflare is lenient once valid session cookies exist. The OS webview is a real browser (not headless) and passes fingerprinting. Eliminating the `headless_chrome` crate shrinks the binary, removes the macOS clone-accumulation bug entirely, and means users do not need Chrome installed.  
**Implication:** `browser_auth.rs` is split: the auth path becomes a Tauri `WebviewWindow` + cookie-extraction flow (lives in `src-tauri/`); the polling path becomes plain `reqwest` calls in `claudar-core`. The `headless_chrome` dependency is removed from the project entirely.

---

### API polling: reqwest with saved session cookies, not headless Chrome

**Decided:** Use `reqwest` with the session cookies saved during auth to poll the Claude.ai usage endpoint directly.

| Option | Pros | Cons |
|--------|------|------|
| **reqwest + saved cookies** ✅ | Lightweight, no browser process, no code-signing clones, fast startup | If Cloudflare tightens its non-browser rules, polling could break (mitigate: refresh cookies via re-auth) |
| headless Chrome for polling | Works today | ~1 GB macOS clone per launch; requires Chrome; slow spawn |

**Why:** Confirmed: usage fetches work cleanly. `reqwest` is already in the dependency tree (indirectly); promoting it to the polling path removes the `headless_chrome` crate and the entire clone-accumulation issue.  
**Implication:** On a 401 / Cloudflare block during polling, the monitor loop emits an `auth-required` event and the UI prompts the user to re-authenticate via the embedded webview. This is the new error-recovery path.

---

## Parking Lot

| Feature | Revisit when |
|---------|-------------|
| Keychain / system credential store | Security audit or user reports for plaintext session concern |
| Windows Chrome detection | Targeting Windows as a first-class distribution platform |
| Dark/light mode theming | shadcn-svelte theming stabilises or user requests it |
| Multiple history instances side-by-side | More than one account is common among users |
| CLI deprecation | If CLI usage drops to near-zero after GUI ships |
| Sparkle / OS update integration | macOS App Store submission |
| Real-time "typing indicator" style tray animation | Fun but cosmetic |
