# Review: Phase 6 — History, Accounts, and Settings Screens

## Fixed during review

- **Removing the last account silently "un-removed" itself.** `remove_instance` allowed
  removing every configured instance. Once `config.instances` became empty,
  `Config::effective_instances()` synthesizes an implicit `"default"` instance again, so
  `get_instances` would immediately show `"default"` reappearing in the Accounts list (with
  "No session", since its files were just deleted) — even though its poll task had been
  stopped. The app would look like the removal silently failed while actually leaving
  monitoring broken (an instance listed in the UI with no running task, and the "No accounts
  configured yet" empty state permanently unreachable). Fixed by rejecting removal when it
  would leave zero configured instances (`src-tauri/src/commands.rs`,
  `remove_instance`): returns `"Cannot remove the only configured account"`, which surfaces
  through the existing `removeError` banner in the confirmation modal.
- **Stale error banner on the History screen after switching tabs.** `selectInstance` only
  cleared `error` when it triggered a fresh `loadHistory` call (i.e., for instances not yet
  cached in `recordsByInstance`). If a load for one instance failed and the user then clicked
  a tab whose data was already cached, the old error banner stayed on screen and hid the
  perfectly good cached chart/stats for the newly selected instance. Fixed by clearing
  `error` when switching to an already-cached instance (`ui/src/routes/History.svelte`,
  `selectInstance`).

Both `cargo check` (src-tauri) and `vite build` (ui) pass after these changes.

## Not fixed — needs decision

- **Settings only exposes "the last" of four configured thresholds, and editing it can
  produce a non-monotonic threshold list.** `ThresholdsConfig` defaults to
  `five_hour: [50, 75, 90, 100]` / `seven_day: [50, 75, 90, 100]` — a list of independent
  warning levels, all of which fire their own notification when crossed
  (`crates/claude-notify-core/src/monitor.rs:470-476`). The Settings screen presents this as
  a single "5-hour limit warning %" / "7-day limit warning %" field, initialized from
  `thresholds.five_hour[length - 1]` (i.e. `100` by default) and saved by replacing only the
  last array element (`c.thresholds.five_hour = [...slice(0, -1), pct]`). Two consequences:
  1. The other three thresholds (`50, 75, 90` by default) keep firing notifications the user
     never sees represented in the UI — they'll get warnings at percentages they didn't
     knowingly configure.
  2. Lowering the displayed value below an earlier entry produces an out-of-order array (e.g.
     `[50, 75, 90, 60]`). Functionally each threshold is still checked independently so this
     doesn't crash anything, but it's a confusing on-disk representation that a user editing
     `config.toml` by hand would find surprising.
  This isn't a quick fix — it needs a product decision on whether Settings should (a) expose
  and edit the full threshold list, (b) collapse the config to a single threshold per limit
  (a data-model/migration change), or (c) keep the current "edit the highest one" behaviour
  but make the existing lower thresholds visible so users aren't surprised by extra
  notifications.

## Open questions

- For the single-threshold simplification in Settings: was the intent that new/onboarding
  users only ever end up with a single-element thresholds array (so "the last element" is
  always "the only element" in practice), making the multi-threshold case above unreachable
  through normal use? If so, it'd be worth confirming the Wizard/setup flow always produces
  a single-entry array, and possibly normalizing legacy multi-entry configs on load — rather
  than relying on Settings to silently mask the extra entries.
