# References

## Tauri 2.0

- [Tauri 2.0 official site](https://v2.tauri.app/) — main docs hub
- [Tauri 2.0 stable release announcement](https://v2.tauri.app/blog/tauri-20/) — changelog, security audit, new plugin system
- [Tauri architecture](https://v2.tauri.app/concept/architecture/) — IPC model, webview, permissions
- [System Tray docs](https://v2.tauri.app/learn/system-tray/) — TrayIconBuilder, platform caveats (Linux mouse events)
- [Window customisation](https://v2.tauri.app/learn/window-customization/) — decorations, sizing, hiding
- [WebviewWindow reference](https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/) — JS API for show/hide/focus
- [Webview versions by platform](https://v2.tauri.app/reference/webview-versions/) — WebKit (macOS/Linux), WebView2 (Windows)
- [Autostart plugin](https://v2.tauri.app/plugin/autostart/) — tauri-plugin-autostart, cross-platform open at login
- [SvelteKit + Tauri setup](https://v2.tauri.app/start/frontend/sveltekit/) — reference for Svelte frontend wiring

## Community Templates

- [tauri2-svelte5-shadcn (alysonhower)](https://github.com/alysonhower/tauri2-svelte5-shadcn) — Tauri 2 + Svelte 5 + shadcn-svelte template with CI/CD for all three platforms
- [tauri2-svelte5-boilerplate (alysonhower)](https://github.com/alysonhower/tauri2-svelte5-boilerplate) — same but with DaisyUI
- [awesome-tauri](https://github.com/tauri-apps/awesome-tauri) — curated apps and plugins

## Charting

- [lightweight-charts-svelte](https://github.com/HuakunShen/lightweight-charts-svelte) — Svelte 5 wrapper for TradingView Lightweight Charts; area/line/histogram series
- [shadcn-svelte LayerChart](https://flowbite-svelte.com/docs/plugins/charts) — built-in chart component added in 2026, D3-based
- [Svelte charting libraries overview](https://awesome.cube.dev/for/svelte/charting-libraries) — comparison of all Svelte chart options

## Tauri GitHub Discussions (multi-window patterns)

- [Creating new windows in v2 #9601](https://github.com/tauri-apps/tauri/discussions/9601) — WebviewWindowBuilder patterns
- [System tray toggle window pattern (Medium)](https://medium.com/@sjobeiri/understanding-the-system-tray-from-concept-to-tauri-v2-implementation-252f278bb57c) — TrayIcon + window show/hide
- [Building a menubar app with Tauri v2 (DEV)](https://dev.to/hiyoyok/building-a-menubar-app-with-tauri-v2-what-nobody-tells-you-2nae) — frameless window + positioning tricks

## Cargo Workspace

- [Cargo workspaces reference](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html) — workspace Cargo.toml, shared deps
- [tauri-build crate](https://crates.io/crates/tauri-build) — build script integration
- [tauri-plugin-autostart GitHub](https://github.com/tauri-apps/tauri-plugin-autostart) — source + issues

## Related Reading

- [DEV: How I Built a Desktop AI App with Tauri v2 + React 19 in 2026](https://dev.to/purpledoubled/how-i-built-a-desktop-ai-app-with-tauri-v2-react-19-in-2026-1g47) — real-world walkthrough
- [Rustify: Build a Desktop App with Tauri v2 in 2026](https://rustify.rs/articles/rust-tauri-v2-desktop-app-tutorial-2026) — step-by-step tutorial
