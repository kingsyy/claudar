# Human to-do — Claudar GUI

Tasks only you can do — external setup, credentials, platform config, decisions. agent-loop
cannot do these.

*Pruned 2026-07-28 during loop-wrapup: Phases 1–8 have all shipped, so the per-phase setup
items below are done, and every open decision has been made. What's left is distribution work
and one piece of documentation.*

## Still open

- [ ] **macOS code signing / notarization** — ad-hoc signing works for personal use (no account
      needed) and is what the current builds use. Distributing to other people needs an Apple
      Developer account. Blocks public release, nothing else.
- [ ] **Document the CLI + GUI double-notify caveat in the README.** The decision was made
      (document it, don't detect it — `03-decisions.md:61`), but no user-facing docs actually say
      so yet. Running the launchd service *and* the GUI app at once sends two of every
      notification.
- [ ] **Runtime verification on a real machine** — notifications, in-app login, start-minimized,
      and the web dashboard (bind address, restart-on-config-change, agent API gating) have not
      been exercised end-to-end on macOS or Windows.

## Done

Setup prerequisites (Tauri CLI, Node ≥ 20, baseline `cargo build`), the Tauri/Svelte scaffold,
the source app icon, and the tray icon set were all completed before their respective phases.
Window sizing landed as resizable with a minimum size. CI shipped as a real GitHub Actions
workflow (`.github/workflows/release.yml`) rather than documentation-only.

## Decisions made

- **CLI `setup` command** → kept but deprecated (`src/cli.rs:17`). Option (b).
- **Default instance name** → `"default"` carries through as-is.
- **Simultaneous CLI + GUI use** → document it; no runtime detection or warning in the GUI.
  (The documentation itself is still outstanding — see "Still open" above.)
- **Welcome copy** → written during Phase 5 ("Never hit a rate limit by surprise"); edit in
  `ui/src/routes/Wizard.svelte` if you want a different tone.
- **Threshold editing UX** → full add/remove list, 1–5 per limit, sorted ascending (Phase 8).
- **Optimistic UI without rollback** → accepted app-wide; see DEVLOG 2026-07-28.
