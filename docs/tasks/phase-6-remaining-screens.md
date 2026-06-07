# Phase 6 — History, Accounts, and Settings Screens

**Depends on:** Phase 5 (onboarding wizard complete; all Tauri commands and events in place)

## Goal

Implement the three remaining screens: History (time-series chart), Accounts (manage instances), and Settings (polling, thresholds, notifications, open-at-login). After this phase the MVP screen set is complete and users can fully configure and manage the app without a terminal.

## In scope

### History screen

- `get_history` Tauri command: reads `sessions/{instance}.jsonl`, filters to the requested `since_days` window, returns `Vec<HistoryRecord>`
- History screen: fetches history on mount via `invoke("get_history", { instance, since_days: 7 })`
- Area chart rendered with LayerChart showing 5h usage over the last 7 days (x = timestamp, y = five_hour_pct)
- Window summary stats below the chart (min, max, avg, number of polls recorded)
- Instance selector (same per-instance tab switcher pattern as Dashboard)
- Empty state when no history exists yet

### Accounts screen

- List all instances with their names and current status (active / no session)
- "Add account" button triggers the onboarding wizard (re-uses the Wizard component from Phase 5, targeting a new instance name)
- `add_instance` Tauri command: creates a new named instance config entry, then triggers auth
- "Remove" button per instance: calls `remove_instance` Tauri command (deletes session + state files for that instance); prompts for confirmation before proceeding
- `remove_instance` Tauri command: removes session and state files for the instance and stops its monitor loop task

### Settings screen

- Polling interval input (minutes; calls `set_config` on change)
- Threshold sliders or inputs for 5h warning % and 7d warning % (per-instance or global as the config supports)
- Notifications on/off toggle
- Open at login toggle (calls `set_autostart` / reads current state via `get_autostart`)
- Settings are applied immediately on change (no separate Save button required, but a confirmation toast is acceptable)

## Out of scope

- Multiple history instances side-by-side (parked in design docs)
- CLI deprecation or removal of service commands (parked)
- Dark/light mode theming (parked)
- App signing and build packaging (Phase 7)

## Data flow

**History:**
```
History.svelte mounts
  → invoke("get_history", { instance, since_days: 7 }) → Vec<HistoryRecord>
  → LayerChart renders area series (timestamp → five_hour_pct)
```

**Accounts remove:**
```
User confirms remove
  → invoke("remove_instance", { instance })
       ↓ Rust: stop monitor task, delete sessions/{instance}.json + state/{instance}.json
  → Accounts list re-fetches via get_instances
```

See `02-architecture.md` — "Data Flow: History query", "IPC Surface".

## Acceptance criteria

- [ ] History screen renders an area chart with data from the last 7 days for the selected instance
- [ ] Chart correctly reflects the JSONL records written by the monitor loop (values match)
- [ ] Accounts screen lists all configured instances and shows their session status
- [ ] Adding an account via "Add account" launches the wizard and the new instance appears in the list after completion
- [ ] Removing an instance prompts for confirmation and then removes it from the list; its monitor task stops; its session and state files are deleted
- [ ] Settings screen displays and saves polling interval, thresholds, and notifications toggle without requiring an app restart
- [ ] Open-at-login toggle on Settings screen has the same effect as the one in the wizard (verified via system autostart registration)
- [ ] All four screens are navigable from the sidebar without any crashes or blank states

## Design references

- See `02-architecture.md` — "Screens & Navigation", "Data Flow: History query", "IPC Surface"
- See `03-decisions.md` — "Charting: shadcn-svelte LayerChart", "Autostart: tauri-plugin-autostart"
