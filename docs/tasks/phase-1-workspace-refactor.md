# Phase 1 — Cargo Workspace Refactor + Remove headless_chrome from Polling

**Depends on:** Nothing (starting point — existing CLI codebase)

## Goal

Convert the project to a Cargo workspace with a `claude-notify-core` library crate, and — as part of that refactor — replace the `headless_chrome` usage in the polling path with plain `reqwest` calls. The `headless_chrome` crate is removed from the project entirely. The CLI keeps working identically. This unblocks the Tauri app from sharing all core logic without carrying in a browser dependency.

## In scope

- Add a top-level `Cargo.toml` declaring a workspace with members: `crates/claude-notify-core`, the CLI bin (root `src/` or `crates/claude-notify-cli`), and a stub `src-tauri/` placeholder (added properly in Phase 2)
- Create `crates/claude-notify-core/` as a `lib` crate; move all existing `src/` modules into it
- Replace `browser_auth.rs`'s polling logic with a new `usage_fetcher.rs` (or equivalent) that uses `reqwest` to call the Claude.ai usage endpoint directly with the session cookies loaded from `sessions/{instance}.json`
  - The cookie file format stays the same; only the transport changes (reqwest, not a Chrome tab)
  - On HTTP 401 or a Cloudflare challenge response, return an `AuthRequired` error variant instead of crashing
- Remove the `headless_chrome` crate from `Cargo.toml` entirely
- The auth portion of `browser_auth.rs` (launching a visible browser for setup) is deleted from core — it moves to `src-tauri/` in Phase 5. The CLI `setup` command can be marked as deprecated or removed from the CLI; confirm with the user (see human-todo)
- `monitor::run_monitor()` must be `pub async fn` callable by the Tauri task later
- Any remaining `eprintln!`/`println!` in core replaced with `tracing` events
- All existing tests pass; `cargo build` and `cargo test` succeed at the workspace root
- `build.rs` (git hash embedding) preserved and working

## Out of scope

- Tauri app creation (Phase 2)
- The auth webview flow (Phase 5)
- History JSONL module (Phase 3)
- Any changes to config, storage paths, notification behaviour

## Data flow

```
monitor_loop (existing)
  UsageFetcher::fetch(cookies)   [reqwest GET to claude.ai usage endpoint]
    → Ok(UsageData)  →  existing threshold / notification logic unchanged
    → Err(AuthRequired)  →  monitor loop logs warning, retries next cycle
                             (Tauri event emitted in Phase 3)
```

Session cookies are read from `sessions/{instance}.json` on each fetch (or cached in memory and refreshed on auth error).

## Acceptance criteria

- [ ] `cargo build` at workspace root succeeds; no `headless_chrome` in the dependency tree (`cargo tree | grep headless` returns nothing)
- [ ] `cargo test` passes all existing tests
- [ ] `claude-notify monitor` fetches real usage data via `reqwest` and fires notifications as before
- [ ] `claude-notify usage` displays correct data (same output as before the refactor)
- [ ] A 401 or blocked response from the API produces a log warning rather than a panic or crash
- [ ] No `eprintln!`/`println!` remain in `claude-notify-core`

## Design references

- See `02-architecture.md` — "Cargo Workspace Layout", "Data Flow: Polling cycle"
- See `03-decisions.md` — "Workspace: Cargo workspace with shared core library", "API polling: reqwest with saved session cookies"

## Skills

/verify
