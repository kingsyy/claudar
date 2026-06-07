# Review: Phase 2 — Tauri App Skeleton

Reviewed 2026-06-07. Both build systems clean: `cargo check` and `npm run build` pass with no warnings.

## Fixed during review

Nothing. No bugs found.

## Not fixed — needs decision

Nothing found.

## Open questions

Nothing. All decisions were already made and documented in DEVLOG.md and memory.

---

### Verification summary

| Check | Result |
|---|---|
| `cargo check` (workspace) | ✓ clean |
| `npm run build` (ui/) | ✓ 231ms, 3 assets |
| `beforeDevCommand` CWD claim | ✓ confirmed via `pwd && ls` probe (DEVLOG) |
| Tauri IPC (`ping` command) | ✓ registered, correct import path |
| Port alignment (devUrl ↔ vite) | ✓ both 1420 |
| Svelte 5 runes syntax | ✓ `$state`, `onclick`, `mount()` |
| Tailwind v4 setup | ✓ `@import "tailwindcss"` via Vite plugin |
| Icon format (RGBA) | ✓ all icons RGBA PNG; inline tray icon also RGBA |
| Window lifecycle (close hides) | ✓ `CloseRequested` → `prevent_close` + `hide` |
| `show_menu_on_left_click(false)` | ✓ updated API used (not deprecated `menu_on_left_click`) |

### Notable implementation choices (no issues, for reference)

- **Tauri version pinned to `=2.11.2`** — required because `"2"` resolves to v2.9.5 which has a trait-mismatch with `wry 0.53.x`. Correct approach; see DEVLOG.
- **`beforeDevCommand = "npm run dev"`** — Tauri sets CWD to the parent of `frontendDist` (`ui/`), so no `--prefix` is needed. Confirmed empirically.
- **Route stubs with phase annotations** — Dashboard, History, Accounts, Settings are present as placeholder components. TODOs reference Phase 4/6, which is accurate per the plan.
- **`testIpc()` fires on mount** — the ping IPC call in `App.svelte:24` runs at startup and logs to console. Fine for a skeleton; remove or gate it before Phase 3 lands real IPC.
