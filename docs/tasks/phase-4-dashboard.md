# Phase 4 — Svelte Dashboard Screen

**Depends on:** Phase 3 (monitor loop emitting `usage-update` events)

## Goal

Implement the Dashboard screen — the first real, useful screen in the app. It shows live 5h and 7d usage gauges, reset countdowns, and a per-instance tab switcher. The gauges update reactively as `usage-update` events arrive, without any user interaction. After this phase the app is genuinely usable as a monitoring tool.

## In scope

- Wire Svelte's Tauri event listener to subscribe to `usage-update` on component mount; unsubscribe on destroy
- Per-instance tab switcher at the top of the Dashboard (one tab per instance returned by `get_instances`)
- For the selected instance: two circular or bar gauges for 5h usage % and 7d usage %, labelled with the raw values
- Reset countdown: human-readable time until the usage window resets (derive from `resets_at` in the payload)
- Predicted burn-rate indicator (show `predicted_pct` from the payload — a simple text stat is fine)
- Loading / empty state when no data has arrived yet (spinner or placeholder)
- Error state when `monitor-error` is received for the selected instance
- The Dashboard is the default/home route — it is shown when the window opens for a returning user
- Sidebar navigation highlights the active route

## Out of scope

- History chart (Phase 6)
- Accounts and Settings screens (Phases 5–6)
- Onboarding (Phase 5)
- Polish, theming, icons beyond functional shadcn-svelte components (Phase 7)

## Data flow

```
Tauri event "usage-update" { instance, five_hour_pct, seven_day_pct, resets_at, predicted_pct }
  → Svelte store (keyed by instance)
  → Dashboard.svelte reactive binding → gauges + countdown re-render
```

On mount: `invoke("get_usage", { instance })` to get initial state before the first poll cycle completes.

See `02-architecture.md` — "Data Flow: Polling cycle" (the Svelte side).

## Acceptance criteria

- [ ] Opening the app window shows the Dashboard with gauges for the default instance
- [ ] Gauges display the correct 5h and 7d percentages matching what `get_usage` returns
- [ ] Switching between instance tabs updates all gauges and countdown to reflect the selected instance
- [ ] Gauges update live when a new `usage-update` event fires (observable within one poll interval)
- [ ] Reset countdown ticks down in real time (re-renders at least every minute without requiring a new poll event)
- [ ] A loading state is shown on first launch before the first usage-update event arrives
- [ ] Monitor error for an instance is surfaced in the UI (not silently swallowed)

## Design references

- See `02-architecture.md` — "Screens & Navigation", "Data Flow: Polling cycle", "IPC Surface"
- See `03-decisions.md` — "Frontend: Svelte 5"
