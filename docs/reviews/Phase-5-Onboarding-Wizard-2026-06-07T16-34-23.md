# Review: Phase 5 — Onboarding Wizard

## Fixed during review

- **Login window could be closed without ever resolving the UI state.** Neither
  `Wizard.svelte` (auth step stuck on "Waiting for login…") nor the re-auth banner
  in `App.svelte` (button stuck on "Connecting…") had any way to recover if the
  user simply closed the embedded `claude.ai/login` webview instead of completing
  the sign-in — `auth-error` was previously only emitted from inside `complete_auth`
  on cookie/org-id failures, never on a user-initiated close. Added a
  `WindowEvent::Destroyed` handler in `start_auth`
  (`src-tauri/src/commands.rs`) that emits `auth-error` with a
  "Login window closed before signing in" message if the window goes away before
  the `completed` flag is set (guarded by the same `AtomicBool` so a normal
  successful close doesn't double-emit).
- **Re-auth banner had no `auth-error` listener**, so the fix above would have
  been silently dropped by the banner anyway. Added an `unlistenAuthError`
  listener in `App.svelte` that resets `reauthing` and surfaces the error message
  in the banner (with a "Try again" label on the retry button), mirroring the
  handling `Wizard.svelte` already had for its own auth step.

All changes build and pass checks:
- `cargo check` / `cargo test` (workspace) — clean
- `npm run build` (ui) — clean

## Not fixed — needs decision

- None found beyond the above. The wizard flow (welcome → login → thresholds →
  done), `start_auth`/`complete_auth` cookie extraction, `set_autostart`/
  `get_autostart`, and the first-run detection in `App.svelte` all look correct
  and consistent with the rest of the codebase.

## Open questions

None.
