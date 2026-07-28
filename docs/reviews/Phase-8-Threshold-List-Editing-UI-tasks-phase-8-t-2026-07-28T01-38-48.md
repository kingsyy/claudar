# Review: [Phase 8 — Threshold List Editing UI](tasks/phase-8-threshold-editing.md) — Users can add/delete 1–5 thresholds per limit in Settings, kept sorted by value

## Fixed during review

Nothing needed fixing. The implementation is correct and consistent with the backend:

- Frontend caps at 5 thresholds, blocks removal below 1, dedupes, and clamps to 0–100 —
  all matching the backend's `ThresholdsConfig::validate_list` (`crates/claudar-core/src/config.rs:119`),
  which was added alongside this change (commit `f7f777a`) and rejects unsorted/duplicate/out-of-range/
  empty/oversized lists on `set_config`.
- Sorting uses an explicit numeric comparator (`.sort((a, b) => a - b)`) everywhere, avoiding the classic
  JS default-lexicographic-sort bug.
- Chip removal and the "Add" input/button are correctly `disabled` at the 1-minimum and 5-maximum
  boundaries, with matching toast messages if those paths are hit programmatically (e.g. via Enter key).
- No stale references to the old CSV parsing (`parseCsvThresholds`, `fiveHourThresholdStr`, etc.) remain
  anywhere in `ui/`.
- Verified: `cargo build --workspace` succeeds, `cargo test -p claudar-core threshold` (20 tests) passes,
  and `npm run build` in `ui/` succeeds with no new warnings (the one pre-existing Svelte warning in
  `Wizard.svelte` is unrelated to this change).

## Not fixed — needs decision

- **Optimistic UI update has no rollback on persist failure.** `addThreshold`/`removeThreshold` update the
  local `fiveHourThresholds`/`sevenDayThresholds` state immediately, then fire `persistThresholds` →
  `persist(...)` without awaiting it. If the `set_config` IPC call fails (disk write error, etc.), the
  error toast fires but the chip list stays showing the un-persisted state until the page is reloaded.
  In practice this should rarely trigger, since the frontend's own guards (max 5, min 1, dedup, 0–100
  range, ascending sort) already satisfy everything the backend validates — but it's worth noting this
  diverges from a strict "don't show state that isn't saved" guarantee. This is the same pattern already
  used elsewhere in this file (e.g. `toggleTrayIcon`, `toggleStartMinimized`), so it's not a regression
  introduced by this task, just an existing app-wide tradeoff that now also applies here.
- **Silent truncation of decimal input.** The "Add threshold" field is `type="number"` with no `step`
  enforcement, so a value like `85.7` is silently truncated to `85` via `parseInt` with no warning. This
  matches the old CSV parser's behavior byte-for-byte, so it's not a new bug, just carried over.

## Open questions

- `docs/todo.md` links this task to `docs/tasks/phase-8-threshold-editing.md`, but that file doesn't exist
  in the repo (unlike Phases 1–7, which all have a corresponding task doc under `docs/tasks/`). Not a code
  issue, but worth creating the doc or fixing the link before marking this phase fully done.
