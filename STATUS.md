---
intent: share
stage: shared
share_target: maintained
next: The in-app updater is fully armed — keypair generated, `plugins.updater.pubkey` filled in, and both `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` set as repo secrets (2026-09-01). None of it has run yet: build two versions back-to-back and confirm an ad-hoc-signed `.app` really does update in place on macOS, and that the universal build's `latest.json` target key matches what the client asks for. After that, runtime-verify the 0.4.6 release — add a ChatGPT account through the built app and confirm the login lands on chatgpt.com (the 0.4.6 fixes went out without this pass); diagnose `decryption failed: aead::Error` on both existing accounts, which currently stops them polling entirely; then account reordering, now rebuilt on pointer events after the HTML5 drop never fired (drag gesture, persistence across restart, tray menu order), the reorganised Settings (Accounts/About tabs, add-account wizard overlay), the Dashboard progressbar semantics with a screen reader, and that the tray icon/menu still update live after the main-thread fix; turn `claudar setup` into a real prompt-and-paste flow (org_id / session_key / cookie, validate via API, write the session file) and rewrite README's stale "## Setup" section with it; runtime-verify the web dashboard (bind/restart-on-config-change/agent API gating), the threshold-list editing UI, and notifications/login/start-minimized on macOS/Windows; code-sign/notarize for distribution. Also runtime-verify the now-full ChatGPT support (5-hour + weekly bars, threshold notifications titled "ChatGPT", history filling in `history/{instance}.jsonl`). Parked behind that: per-account thresholds (`[thresholds]` is still global), and the `Gauge`/`UsageProvider` generalisation in `docs/04-multi-provider.md`
blocker: null
updated: 2026-09-14
---

# Claudar — Monitor Claude.ai usage limits with native desktop notifications

## What it is

A lightweight Rust daemon that fetches real-time Claude.ai usage data (bypassing local-only tracking) and sends native desktop notifications when approaching rate limits. Supports multiple accounts, configurable thresholds, and runs as a background service on macOS and Linux.

## Current state

**GUI app is functionally complete.** All 8 build phases are done (Phase 1: workspace refactor, Phase 2: Tauri skeleton, Phase 3: monitor loop integration, Phase 4: Dashboard screen, Phase 5: wizard, Phase 6: History/Accounts/Settings, Phase 7: icons/builds, Phase 8: threshold list editing). The Tauri app bundles the monitor loop and runs it as a background tokio task. Screens include Dashboard (5h/7d gauges, reset countdowns, predicted burn), History (SVG chart, 7-day stats), Accounts (multi-account list, add/remove with wizard re-use), Settings (poll interval, thresholds, notifications toggle, autostart). Wizard guides new users through login (in-app webview auth), threshold setup, and autostart toggle.

**Phase 8 shipped and wrapped up (2026-07-28).** Settings previously exposed only the highest of 4 thresholds per limit, which produced non-monotonic configs and surprise notifications from hidden thresholds. It now edits the full list: add/remove 1–5 thresholds per limit, kept sorted ascending and deduped, with matching backend validation in `ThresholdsConfig::validate_list` enforced on `set_config`. The wrap-up pass accepted the app-wide optimistic-UI-without-rollback pattern as a deliberate tradeoff (see DEVLOG), added `step="1"` to the threshold inputs, and pruned the stale `docs/human-todo.md`.

**2026-08-12:** The CLI `setup` command is **no longer deprecated** — it's the supported minimal /
headless path for users with no desktop session, and its banner now points at the shipped GUI wizard
rather than promising one. The README documents that running the launchd/systemd service alongside
the GUI app double-notifies (two monitors, one account); the GUI can't detect this itself because
`is_service_running()` isn't exposed as a Tauri command. Tracked follow-up: `setup` still only
*prints* instructions — it should prompt-and-paste and write the session file.

No feature work remains in the build plan. What's left is distribution and verification — see `docs/human-todo.md`.

**ChatGPT is a full account as of 2026-08-31 (unreleased).** The account started reporting a 5-hour
window alongside the weekly one — the same pair Claude reports — so ChatGPT now runs through the
same code as Claude: both bars on the Dashboard, tray and web dashboard, threshold/reset/capacity
notifications (titled with the provider name), pace prediction, and a history record per poll, which
makes the History screen work for it unchanged. Window lengths and labels come from the API's
`limit_window_seconds`, and an account that reports only a weekly window has its 5-hour bar omitted
rather than drawn at 0%. The `Gauge` generalisation stays parked — see DEVLOG 2026-08-31. Not yet
runtime-verified against the live account.

**ChatGPT weekly bar shipped as 0.4.6 (2026-08-30).** A second provider (`openai-web`) renders one
weekly usage bar on the Dashboard alongside Claude's two. Auth is a browser login that mints a
bearer from the session cookie — no OAuth, no Codex install (see DEVLOG 2026-08-27). Display-only:
no notifications, no history, no `Gauge` generalisation. The 0.4.5 build of this shipped with a
Claude-only login path that sent ChatGPT accounts to claude.ai and trapped the UI in a wizard with
no cancel; 0.4.6 fixes that and replaces the Dashboard's "open Settings to sign in" text with a
**Log in now** button. **The 0.4.6 release was cut without a runtime pass** — the ChatGPT login has
not been exercised in a built app.

**In-app updater built, not yet armed (2026-08-31).** `tauri-plugin-updater` now checks GitHub
Releases and installs from Settings → About, with a "What's new" panel fed by a new `CHANGELOG.md`
(backfilled 0.3.0–0.4.6). CI extracts the tagged version's changelog section into both the GitHub
release body and `latest.json`, so the two can't drift. Signing is armed: the minisign keypair was generated 2026-08-31,
its public half is in `tauri.conf.json`, and both `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are repo secrets as of 2026-09-01. Compiles clean and the full
suite passes (164 tests), but **no release has been cut with any of this** — the signing path, the
generated `latest.json`, and the About panel itself are all unexercised.

**Both live accounts are failing to poll (2026-08-30).** `decryption failed: aead::Error` on
`personal` and `work` in a dev run — a session-key/keychain mismatch, unrelated to the provider
work, undiagnosed.

**Multi-provider researched, not started (2026-08-18).** `docs/04-multi-provider.md` records the
research and two decisions for monitoring OpenAI/Codex alongside Claude. OpenAI's
`backend-api/wham/usage` returns the same shape Claudar already models (percent-of-window +
reset timestamp), so the work is a core-model generalisation — `UsagePayload`/`LimitType` become a
keyed `Vec<Gauge>` behind a `UsageProvider` trait — not a rewrite; the provider-agnostic
infrastructure stays. Sequenced *after* 0.4.5, and behind a no-behaviour-change split of
`History.svelte` (1908 lines, chart hardcoded to two series). Codex auth would be Claudar's own
PKCE OAuth flow. One live call is still needed to pin the minimal header set.

`cargo tauri build` produces a working `.app` locally (verified by launching). Code-signing/notarization (for distribution outside the dev machine) is out of scope for this phase but required before real release.

**v0.4.1 (2026-06-22):** Settings/UX pass — start-minimized-at-login (gated to login launches via a `--minimized` autostart flag, with a macOS dock-Reopen handler and off-screen-window recentering), accessible On/Off toggles (text + ✓/✕ + colour + position, colourblind/RTL-safe), poll-interval changes now preserve history retention in days with a confirming message, and an overuse warning below 5-minute intervals. Plus the About panel (version/build/license/source). Built and shipped as `Claudar_0.4.1_aarch64.dmg`; start-minimized + dock-Reopen not yet runtime-verified on a real login.

**v0.4.0 (2026-06-22):** cross-platform hardening pass — rewrote browser detection (machine-wide + per-user locations for Chrome/Edge/Brave/Chromium with a `$PATH` fallback), unified the notifier onto `tauri-plugin-notification` for all OSes, plus a Dashboard header redesign. `cargo check`/core tests green; GUI notification + login paths on real macOS/Windows still need a manual runtime check.

**Navigation and accessibility pass (2026-08-13, unreleased):** Top-level nav is down to Dashboard / History / Settings — Accounts and About are now Settings tabs, and the old "Unused Capacity" tab folded into Notifications next to the pre-reset reminders it largely duplicates (with a warning when both are set for the same limit, since both fire). New text-size preference (small→larger) that scales the root font size, `role="progressbar"` on the Dashboard usage bars, an ARIA tablist for the Settings tabs, and a CSS-level `prefers-reduced-motion` fallback. Every text/background pair measures WCAG AA or better in all three themes. See DEVLOG 2026-08-13. Screenshot-verified against the vite dev server; screen-reader and bundled-app behaviour still want a manual check.

**Tray crash fixed (2026-08-13, unreleased):** v0.4.4 crashed after 6 days up (SIGTRAP, `claudar-app-2026-08-13-011938.ips`). The poll task was mutating the tray from a tokio worker, racing the non-atomic `Rc` refcount inside `tray_icon::TrayIcon` that Tauri only marks `Send` on the promise it stays on the main thread; a lost update drove it to zero and the status item was torn down off-thread. `set_tray_icon`/`refresh_tray_menu` now dispatch via `run_on_main_thread`. See DEVLOG 2026-08-13. Builds and tests clean; live tray behaviour still wants a manual check.

## Next

- **Option A:** Install the 0.4.6 release and walk the ChatGPT flow end to end — add an account,
  confirm the browser opens chatgpt.com, confirm the weekly bar renders. This is the pass 0.4.6
  shipped without.
- **Option B:** Diagnose `decryption failed: aead::Error` on `personal` and `work`. Neither account
  is polling until this is fixed, and it predates the provider work.
- **Option C:** Code-sign/notarize for distribution outside this machine (`docs/building.md`), which
  is what still separates the CI release assets from something a stranger can open without
  right-click → Open.
