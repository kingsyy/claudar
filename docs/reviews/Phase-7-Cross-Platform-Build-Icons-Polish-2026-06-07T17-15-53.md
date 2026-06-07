# Review: Phase 7 — Cross-Platform Build + Icons + Polish

## Fixed during review

Nothing needed fixing. `cargo build --release` compiles cleanly with the new
`tauri-plugin-window-state` dependency and `image-png` feature.

## Not fixed — needs decision

Nothing found that needs a developer decision.

## Open questions

None. Verification performed:

- **Tray icons**: the 5 PNG variants (`tray-{green,yellow,orange,red,grey}.png`, 22×22 RGBA)
  decode correctly via `Image::from_bytes` and their pixel colours
  (`(0,180,0)`, `(200,180,0)`, `(220,120,0)`, `(220,0,0)`, `(128,128,128)`) match both the
  previously hardcoded RGBA placeholders and the level→colour table in `docs/03-decisions.md`
  (`Tray Icon: Status colour encoding`).
- **`monitor_loop::set_tray_icon`**: now decodes PNG bytes and logs+returns on a decode error
  instead of constructing a raw buffer — reasonable degradation path.
- **`tauri.conf.json`**: bundle is now `active: true` with explicit per-platform targets
  (`app`/`dmg`, `deb`/`appimage`, `msi`/`nsis`), the icon list points at real generated assets
  (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`), and the identifier
  was corrected from `com.claude-notify.app` (which Tauri flags as conflicting with the `.app`
  bundle extension on macOS) to `com.avr.claude-notify`.
- **`tauri-plugin-window-state`**: registered with `POSITION | SIZE` flags. The plugin's
  generated permission set (`window-state:default` etc.) appears in
  `gen/schemas/{desktop,macOS}-schema.json` but is not added to
  `capabilities/default.json` — this is correct as-is, since the plugin saves/restores window
  state from Rust-side setup/event hooks rather than via frontend-invoked commands, so no
  capability grant is required for its automatic behaviour.
- **`docs/building.md`**: prerequisites, per-platform build steps, output paths, and CI matrix
  sketches are consistent with the configured bundle targets and the actual generated icon
  filenames.

Everything in this phase looks good.
