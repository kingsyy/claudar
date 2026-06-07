# Review: Phase 1 — Cargo Workspace Refactor

Covers commits `b47d4fc` (extract claude-notify-core) and `2cafa4b` (remove migrated src/ files).

Build: `cargo build --all` clean. `cargo check --all` clean. `cargo test --all` — 106 tests, 0 failures (36 in the binary crate, 70 in the library crate).

## Fixed during review

Nothing. No critical bugs or broken behaviour were found.

## Not fixed — needs decision

### Unused `blocking` reqwest feature in root Cargo.toml

`reqwest` in the binary's `Cargo.toml` declares the `blocking` feature, but no code in `src/` calls `reqwest::blocking::*`. The core crate correctly omits `blocking`.

- **Impact**: Longer incremental compile times (compiles the blocking thread-pool machinery), larger binary.
- **Fix**: Remove `"blocking"` from the `features` list in root `Cargo.toml` if no CLI command needs synchronous HTTP. If a future command does, add it back then.
- **Location**: `Cargo.toml:21`, `features = ["json", "cookies", "blocking", "rustls-tls", "gzip", "brotli"]`

## Open questions

None. The extraction is complete and structurally sound.

---

**Checklist summary**

| Area | Status |
|---|---|
| Workspace manifest (resolver, members) | OK |
| Dependency versions consistent across crates | OK |
| All 11 core modules moved to `claude-notify-core` | OK |
| All modules `pub`-exported from `lib.rs` | OK |
| Binary imports from `claude_notify_core::*` | OK |
| `run_monitor()` converted to `async`, correctly awaited | OK |
| `AuthRequiredError` downcasting in monitor loop | OK |
| `setup` command deprecated stub compiles and routes | OK |
| All tests pass (36 binary + 70 core) | OK |
| `reqwest` blocking feature unused in binary | Minor (see above) |
