# Phase 5 — Onboarding Wizard

**Depends on:** Phase 4 (Dashboard working for returning users)

## Goal

Add a welcoming, transparent first-run experience. The wizard opens automatically when no session files exist. It guides the user from a warm welcome through an in-app login (no Chrome required — login happens inside a Tauri webview, inside the app), threshold configuration, and an open-at-login prompt. The experience is friendly, non-technical, and honest about what the app does with data. Re-auth triggered by `auth-required` events (expired session) uses the same auth webview.

## In scope

### First-run detection

Check on launch whether any instance has a valid session file. If none, render the Wizard in place of the main window (sidebar hidden). If a session exists, go directly to Dashboard.

### Wizard steps

**Step 1 — Welcome**

- Warm, plain-language headline: something like "Keep an eye on your Claude usage" (not technical)
- 2–3 sentence explanation: what the app watches, that it runs quietly in the menu bar, that no messages or content are ever read — only usage percentages
- A collapsible "How does this work?" section with a short paragraph: it logs into Claude.ai on your behalf using your browser session, checks a usage number periodically, and that's it
- A collapsible "Where is my data?" section: session cookies stored locally at `~/.config/claude-notify/sessions/`, never uploaded, never shared. Usage numbers stored locally at the same path for the history chart. Nothing leaves the device.
- Primary button: "Get started"

**Step 2 — Connect your account**

- Headline: "Log in to Claude.ai"
- Plain explanation: "A Claude.ai login window will open below. Sign in as you normally would — we'll detect when you're logged in and close the window automatically."
- A Tauri `WebviewWindow` (labelled `"auth"`) opens pointing at `https://claude.ai/login` — this window is embedded or modal, not a floating Chrome window. It looks like part of the app.
- While waiting: show a subtle animated status line ("Waiting for login…") below the webview
- On `auth-complete` event: close the webview, show a brief success state ("Connected ✓"), advance to Step 3 after 1 second
- On `auth-error` or timeout: show an inline error with a "Try again" button (re-opens the webview)
- A "Why do I need to log in?" link opens a short explanation in a sheet/modal: the app needs your session cookie to check your usage — the same token your browser uses. It's read-only. You can revoke it by logging out of Claude.ai.

**Step 3 — Configure thresholds**

- Headline: "When should we notify you?"
- Two sliders or inputs: "Notify me when 5-hour usage reaches __%" and "Notify me when 7-day usage reaches __%"
- Defaults pre-populated (80% and 80%, or whatever the current config defaults are)
- Brief tooltip/label on each: "Your 5-hour usage window resets every 5 hours. Claude stops responding when it hits 100%."
- Primary button: "Save and continue"
- Calls `set_config` with the chosen values before advancing

**Step 4 — Done**

- Headline: "You're all set"
- One-line confirmation of what's active: "Monitoring [account email or 'your Claude account'] · checking every [N] minutes"
- Open at login toggle with a plain label: "Start automatically when I log in" — calls `set_autostart` when toggled
- Primary button: "Open dashboard"
- Clicking the button closes the wizard and shows the Dashboard; the monitor loop starts

### Auth webview (`start_auth` command)

- In `src-tauri/commands.rs`: `start_auth` opens a `WebviewWindow` pointed at `https://claude.ai/login`
- A Tauri navigation event listener watches for successful navigation to `https://claude.ai` (post-login URL)
- On that navigation, extract session cookies from the webview's cookie store via Tauri's cookie API
- Save cookies to `sessions/{instance}.json`
- Close the auth window
- Emit `auth-complete { instance }` or `auth-error { instance, message }`

### Re-auth flow (expired session)

- When the monitor loop emits `auth-required { instance }`, the main window shows a non-blocking banner: "Your Claude session has expired — [Re-connect]"
- Clicking Re-connect opens the same auth webview flow inline
- After `auth-complete`, the banner dismisses and monitoring resumes on the next poll cycle

### Tauri commands added this phase

- `start_auth { instance }` — opens auth webview, emits `auth-complete` or `auth-error`
- `set_autostart { enabled: bool }` — wired to `tauri-plugin-autostart`
- `get_autostart` → `bool`

## Out of scope

- Accounts screen "Add account" button (Phase 6 wires that; the wizard component is reused)
- History, Accounts, Settings screens (Phase 6)
- Final icon/illustration assets (Phase 7)
- The auth webview does not need to look polished — functional and clearly labelled is enough

## Data flow

```
Step 2: Svelte invokes start_auth
  → Tauri opens WebviewWindow("auth", "https://claude.ai/login")
  → Tauri navigation listener fires on successful login (URL changes to claude.ai home)
  → cookies extracted from webview cookie store
  → session saved to sessions/{instance}.json
  → emit("auth-complete", { instance })
  → Svelte advances to Step 3
```

```
Monitor detects expired session
  → emit("auth-required", { instance })
  → Svelte shows re-auth banner
  → user clicks Re-connect → same start_auth flow
  → auth-complete → banner dismisses
```

See `02-architecture.md` — "Data Flow: Onboarding auth flow", "IPC Surface".

## Acceptance criteria

- [ ] Fresh install (no session files) shows the wizard on launch; sidebar is hidden
- [ ] Step 1 welcome screen renders clearly; "How does this work?" and "Where is my data?" sections expand in place
- [ ] Step 2 opens a Claude.ai login page inside the app (no Chrome process spawned — verify with `ps`)
- [ ] Successfully logging in advances the wizard to Step 3 automatically (no user button press needed)
- [ ] An auth error shows inline with a working "Try again" button
- [ ] Step 3 sliders/inputs save correct values to config before advancing
- [ ] Step 4 open-at-login toggle registers correctly with the OS (verify app survives a reboot when enabled)
- [ ] Completing the wizard shows the Dashboard with live data from the newly connected account
- [ ] Returning user (valid session) skips the wizard entirely
- [ ] An `auth-required` event during normal monitoring shows the re-auth banner in the main window; re-authenticating via the banner restores monitoring without a restart

## Design references

- See `02-architecture.md` — "Screens & Navigation", "Data Flow: Onboarding auth flow", "IPC Surface"
- See `03-decisions.md` — "Auth: Tauri embedded webview", "Onboarding: Wizard shown on first launch", "Autostart: tauri-plugin-autostart"
