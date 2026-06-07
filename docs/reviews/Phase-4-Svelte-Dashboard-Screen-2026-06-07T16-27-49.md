# Review: Phase 4 — Svelte Dashboard Screen

## Fixed during review

- **Loading spinner stuck alongside error banner.** When `get_usage` failed for the
  selected instance (e.g. no session configured), `usage` stayed `undefined` while
  `error` was set, so the template rendered both the error banner *and* the
  "Waiting for the first usage update…" spinner — and the spinner never went away
  since there's nothing left to trigger a re-render of `usage`. Changed the
  conditional in `Dashboard.svelte` so the loading state only shows when there is
  no usage *and* no error (`{#if !usage && !error}` … `{:else if usage}`).
- **Dead CSS rule.** `.gauge::before { content: ""; position: absolute; }` had no
  visual effect (no positioning context, dimensions, or background) — looked like
  a leftover from an earlier ring-mask approach to the gauge. Removed it.

Both the Vite build (`npm run build`) and `cargo check` pass after these changes.

## Not fixed — needs decision

- `InstanceInfo.has_session` is sent from the backend (`commands.rs`) and declared
  in the frontend's `UsagePayload`-adjacent type, but never read in `Dashboard.svelte`.
  The dashboard will still call `get_usage` for instances without a session, which
  surfaces as an error banner. An alternative would be to short-circuit and show a
  "no session — run setup" message instead of attempting a fetch that's guaranteed
  to fail. Leaving this to the developer since it depends on what UX is intended for
  unauthenticated instances elsewhere in the app.

## Open questions

None — the rest of the implementation (event subscriptions, tab switching, gauge
rendering, countdown formatting, predicted-burn stat) looks correct and matches the
IPC contract exposed by `commands.rs`/`monitor.rs`.
