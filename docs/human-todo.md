# Human to-do — Claudar GUI

Tasks only you can do — external setup, credentials, platform config, decisions. agent-loop
cannot do these.

*Pruned 2026-07-28 during loop-wrapup: Phases 1–8 have all shipped, so the per-phase setup
items below are done, and every open decision has been made. What's left is distribution work
and runtime verification.*

> An unticked box is not proof of undone work — it may be genuinely open, or simply not
> verifiable from this machine. Check before repeating one.

## Still open

- [ ] **macOS code signing / notarization** — ad-hoc signing works for personal use (no account
      needed) and is what the current builds use. Distributing to other people needs an Apple
      Developer account. Blocks public release, nothing else.
- [ ] **Runtime verification on a real machine** — notifications, in-app login, start-minimized,
      the threshold-list editing UI, and the web dashboard (bind address,
      restart-on-config-change, agent API gating) have not been exercised end-to-end on macOS or
      Windows. Also confirm the tray icon/menu still update live after the 2026-08-13
      main-thread fix.

## Done

Setup prerequisites (Tauri CLI, Node ≥ 20, baseline `cargo build`), the Tauri/Svelte scaffold,
the source app icon, and the tray icon set were all completed before their respective phases.
Window sizing landed as resizable with a minimum size. CI shipped as a real GitHub Actions
workflow (`.github/workflows/release.yml`) rather than documentation-only.

Re-confirmed 2026-08-10 from direct local evidence: `cargo tauri --version` → `tauri-cli 2.11.2`;
`node`, `npm`, and `pnpm` all on `$PATH`; `cargo build` exited 0; `src-tauri/Cargo.toml` and
`ui/package.json` both present.

## Decisions made

- **CLI `setup` command** → kept, and **un-deprecated 2026-08-12** as the supported minimal /
  headless path for machines with no desktop session (`src/cli.rs`).
- **Default instance name** → `"default"` carries through as-is.
- **Simultaneous CLI + GUI use** → document it; no runtime detection or warning in the GUI.
  Documented in the README 2026-08-12; the GUI still can't warn, because
  `is_service_running()` (`src/service.rs`) isn't exposed as a Tauri command.
- **Welcome copy** → written during Phase 5 ("Never hit a rate limit by surprise"); edit in
  `ui/src/routes/Wizard.svelte` if you want a different tone.
- **Threshold editing UX** → full add/remove list, 1–5 per limit, sorted ascending (Phase 8).
- **Optimistic UI without rollback** → accepted app-wide; see DEVLOG 2026-07-28.
