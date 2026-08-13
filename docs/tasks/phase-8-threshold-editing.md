# Phase 8 — Threshold List Editing UI

**Depends on:** Phase 6 (Settings screen implemented)

> Written retroactively during loop-wrapup, after the phase shipped in commits
> `f7f777a` (backend validation) and `2f41ca7` (frontend UI). Recorded here so the
> `docs/todo.md` link resolves and Phase 8 matches the format of Phases 1–7.

## Goal

Settings exposes the *full* threshold list for each limit, not just the highest entry.
Users can add and remove individual thresholds (1–5 per limit), and the stored list is
always sorted ascending with no duplicates.

This closes the issue raised in the Phase 6 review: Settings previously showed a single
"warning %" field initialised from `thresholds.five_hour[len - 1]` and saved by replacing
only that last element. The other configured thresholds kept firing notifications the user
never saw represented in the UI, and lowering the displayed value could produce a
non-monotonic array like `[50, 75, 90, 60]`.

## In scope

- **Backend validation** (`crates/claudar-core/src/config.rs`): `ThresholdsConfig::validate_list`
  rejects empty lists, lists longer than 5, values above 100, and any list that is not
  strictly increasing (i.e. unsorted or containing duplicates). `validate()` applies this to
  both `five_hour` and `seven_day`. Enforced on `set_config`.
- **Frontend list UI** (`ui/src/routes/Settings.svelte`): each limit renders its thresholds as
  a list of chips with a per-chip remove button, plus an "Add threshold" number input and button.
- **Guards mirroring the backend**: add is disabled at 5 entries, remove is disabled at 1 entry,
  duplicates are rejected, values are clamped to 0–100, and the list is re-sorted with an explicit
  numeric comparator (`.sort((a, b) => a - b)`) after every mutation. Toast messages cover the
  paths reachable programmatically (e.g. the Enter key).
- **Removal of the old CSV representation**: `parseCsvThresholds`, `fiveHourThresholdStr` and
  friends deleted; no stale references left in `ui/`.

## Out of scope

- Collapsing the data model to a single threshold per limit (rejected — multiple warning
  levels are the intended behaviour).
- Migrating existing on-disk configs that violate the new rules. Legacy multi-entry configs
  written by earlier versions are already sorted in practice; `validate()` only gates writes
  via `set_config`, not reads.
- Rollback of the optimistic UI update if `set_config` fails — accepted as an app-wide
  pattern, see DEVLOG (2026-07-28).

## Data flow

`Settings.svelte` local state (`fiveHourThresholds` / `sevenDayThresholds`)
→ `persistThresholds` → `persist(...)` → `set_config` IPC
→ `ThresholdsConfig::validate()` → `config.toml`.

The monitor loop reads the list unchanged; each entry is still checked independently
(`crates/claudar-core/src/monitor.rs`), so no monitor-side change was needed.

## Acceptance criteria

- [x] Settings shows every configured threshold for both limits, not just the last one
- [x] A threshold can be added, up to a maximum of 5 per limit
- [x] A threshold can be removed, down to a minimum of 1 per limit
- [x] The persisted list is always sorted ascending with no duplicates
- [x] `set_config` rejects an invalid list rather than writing it
- [x] `cargo build --workspace` succeeds; `cargo test -p claudar-core threshold` passes
- [x] `npm run build` in `ui/` succeeds with no new warnings

## Design references

- See `03-decisions.md` — threshold configuration
- See `reviews/Phase-6-History-Accounts-and-Settings-Screens-2026-06-07T16-58-23.md` —
  the "Not fixed — needs decision" entry that motivated this phase
