## 2026-06-07 · Phase 2 — Tauri 2.0 app skeleton + Svelte 5 frontend scaffold

**What:** Added `src-tauri/` (Tauri 2.11.2, `claude-notify-app` crate) and `ui/` (Svelte 5 + Vite 6 + Tailwind CSS v4 + shadcn-svelte config). Grey 16×16 RGBA tray icon, context menu (Show/Hide/Quit), left-click toggle, close-button hides window. Stub `ping` IPC command. Sidebar scaffold with four nav routes (Dashboard, History, Accounts, Settings). Builds clean; `cargo tauri dev` launches the app.

**Why:** Tauri 2.11.2 pinned explicitly — resolving `tauri = "2"` picked v2.9.5 which had a trait-mismatch bug with `wry 0.53.5` (`eval_script_with_callback` not implemented). Svelte 5 runes used for state (`$state`). `beforeDevCommand` path: Tauri sets CWD to the parent of `frontendDist` (i.e. `ui/`), so `npm run dev` works without `--prefix` — confirmed via `pwd && ls` probe. Icons regenerated as RGBA (color type 6) after `generate_context!()` rejected RGB PNGs.

**Next:** Phase 3 — wire `run_monitor()` into a Tauri tokio task, emit `usage-update` events to the Svelte frontend.
