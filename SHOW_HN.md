# Show HN draft — Claudar

## Title

> Show HN: Claudar – A desktop app that tracks your real Claude.ai usage limits

## Post body

Claudar is a small Rust + Tauri desktop app that monitors your Claude.ai usage limits and sends native notifications before you hit a wall.

The reason I built it: Claude Code's built-in `/usage` only knows about usage tracked *locally*. If you bounce between machines, a browser session, and the desktop app on the same account — like I do — that number is wrong. Claudar fetches your actual usage straight from claude.ai, so the 5-hour and 7-day numbers reflect everything, not just what one device happened to see.

What it does:

- Live dashboard with 5-hour and 7-day gauges, reset countdowns, and a "predicted burn" warning if your current pace will blow past 100% before the window resets
- Native desktop notifications at configurable thresholds (50/70/90% by default), on overage prediction, and on reset
- History view with a usage chart and weekly pace
- Multiple accounts (work + personal) monitored side by side
- Runs in the background; can start minimized at login

How it works: a one-time login happens in a real browser over the Chrome DevTools Protocol (this gets past Cloudflare's bot detection cleanly). After that, ongoing polling is just `reqwest` with the stored cookies — no browser running in the background. Session cookies are auth tokens, so they're encrypted at rest with AES-256-GCM; the key lives in your OS keychain (with a 0600 key-file fallback), scoped to a single named item.

macOS, Linux, and Windows builds are on the Releases page. Full disclosure: the binaries aren't code-signed/notarized yet, so Gatekeeper/SmartScreen will warn on first launch — install instructions cover the workaround, or you can build from source (Rust + a Chromium browser for the login step).

Also worth being upfront about: this was largely vibe-coded as a personal tool, so treat it as such. It scratches my own itch well, but I'd love feedback on the approach — especially the auth/cookie handling and anything I've gotten wrong about the usage API.

Repo: https://github.com/kingsyy/claudar

## Follow-up comment (post as the first reply once it's up)

A bit more backstory and some technical detail for anyone curious:

The thing that pushed me over the edge was repeatedly getting surprised by a rate limit mid-task with no warning, because `/usage` was only counting one device. Once I realized the real numbers were sitting behind an authenticated claude.ai endpoint, the rest was just plumbing.

The auth flow is the part I went back and forth on the most. The naive approach — scripting a headless browser for every poll — is heavy and trips Cloudflare. What I landed on: drive a real Chromium instance over the Chrome DevTools Protocol *once* during login, pull the session cookies (`sessionKey`, `cf_clearance`, etc.), then do all ongoing polling with plain `reqwest`. So the browser only exists for the one-time login; after that it's a lightweight background task polling every 15 minutes by default.

On storing the cookies: they're effectively account credentials, so they're encrypted at rest with AES-256-GCM. The key goes in the OS keychain (Keychain / Secret Service / Credential Manager) scoped to a single named item, with a `0600` key-file fallback when no keychain backend exists. Claudar never reads any other keychain entry.

It started as a CLI and grew a Tauri GUI on top; the monitor loop is shared and runs as a tokio task inside the app. Stack is Rust end to end, Tauri for the shell, `tauri-plugin-notification` for cross-platform notifications.

Honest caveats I'd rather you hear from me:

- It relies on an undocumented claude.ai endpoint, so Anthropic could change or break it at any time. If they ship an official usage API, I'd happily switch to it.
- No code-signing/notarization yet, so first launch needs the Gatekeeper/SmartScreen workaround.
- It was largely vibe-coded for my own use — works well for me, but it hasn't been battle-tested across a lot of setups.

Very open to feedback on any of this — especially if you've poked at the usage API yourself or have thoughts on the cookie handling.
