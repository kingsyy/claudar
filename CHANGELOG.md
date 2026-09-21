# Changelog

All notable user-facing changes to Claudar are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Versions 0.4.3 and 0.4.5 were internal version bumps that were never tagged or
> published. Their changes ship as part of 0.4.4 and 0.4.6 respectively.

## [Unreleased]

## [0.4.7] — 2026-09-21

### Added

- **ChatGPT accounts are now first-class.** Both of ChatGPT's rolling windows —
  the 5-hour and the weekly one — appear on the Dashboard, the tray menu and the
  web dashboard, with the same pace projection, threshold notifications, reset
  alerts and recorded history that Claude accounts get. Notification titles name
  the service the alert is about.
- In-app updater. Claudar checks GitHub for new releases and can download and
  install them for you from **Settings → About**, showing what changed before
  you commit to the update.
- **Custom pace zones.** The colored bands on each usage bar (on pace, sweet
  spot, over pace, critical) are now editable per zone — thresholds and colors —
  instead of fixed tiers.

### Fixed

- **ChatGPT login was never detected.** The login window would complete, but
  Claudar kept waiting: it looked for the session cookie under one exact name,
  and ChatGPT's auth provider splits that cookie into numbered chunks once the
  token gets large. Detection now recognizes the chunked form and reassembles it.

## [0.4.6] — 2026-08-30

### Added

- **ChatGPT usage bar.** Add a ChatGPT account and its weekly usage appears on the
  Dashboard next to Claude's 5-hour and 7-day limits. Display-only for now — no
  notifications and no history for ChatGPT yet.
- **Drag-and-drop account ordering.** Reorder accounts on the Accounts screen; the
  order persists across restarts and is reflected in the tray menu.
- A text-size preference, and reduced-motion support for animations.

### Changed

- Accounts and About are now tabs inside Settings rather than separate screens, and
  capacity settings moved into Notifications.
- The "super dark" theme is now true black.
- Adding an account is a wizard overlay instead of a full-screen takeover.
- The `claudar setup` CLI command is no longer deprecated — it is the supported path
  for machines with no desktop session.

### Fixed

- **ChatGPT accounts were sent to the claude.ai login** instead of the ChatGPT one, and
  the wizard had no way to cancel out, trapping the UI.
- The Dashboard's "open Settings to sign in" text is now a **Log in now** button.
- The tray icon and menu could stop updating; all tray access now happens on the main
  thread.
- Dashboard usage bars now report progressbar semantics to screen readers.

## [0.4.4] — 2026-07-25

### Added

- **Read-only web dashboard.** An opt-in local HTTP endpoint for checking usage from
  another machine or a script. Off by default; enable it under `[web]` in `config.toml`.
- Hover tooltips on the History heatmap.

### Fixed

- Reset detection in History was wrong, causing incorrect window boundaries. The Resets
  view was rebuilt.
- Session decryption no longer silently re-keys when the keychain read fails — a failed
  read is now an error instead of quietly discarding your saved session.
- Cloudflare-blocked requests are retried immediately over a persistent HTTP client, with
  a two-tier bypass (native TLS first, webview fallback) instead of failing the poll.

## [0.4.2] — 2026-06-26

### Fixed

- Cloudflare transport errors are now reported clearly rather than as a generic failure,
  and escalate to "re-login needed" after three consecutive failures instead of retrying
  silently forever.

### Documentation

- README gained Dashboard and History screenshots, the real release bundle filenames, and
  the macOS Gatekeeper workaround steps.

## [0.4.1] — 2026-06-22

### Added

- **About panel** showing version, build, and license.
- **Start minimized at login** — launch into the tray without a window flash.
- Accessible Settings toggles (keyboard reachable, correctly labelled).
- Real cross-platform release bundles built by CI: `.dmg`, `.msi`, `.exe` (NSIS),
  `.deb`, and `.AppImage`.

## [0.3.0] — 2026-05

The first release of the desktop app. Claude Notify became **Claudar**.

### Added

- Native desktop app (Tauri) with Dashboard, History, Accounts, and Settings screens,
  replacing the CLI-only workflow.
- Multi-account support with per-account re-login.
- In-app browser login flow, replacing manual cookie pasting.
- **Session files are now encrypted at rest** (AES-256-GCM), with the key in the OS
  keychain.
- History view with a dual-series chart, usage prediction, and weekly pace.
- Dark mode, a new radar-pulse logo, and a full app/tray icon set.

## [0.2.0] and [0.1.0]

Early CLI-only releases: usage polling, threshold notifications, and launchd/systemd
service installation.

[Unreleased]: https://github.com/kingsyy/claudar/compare/v0.4.6...HEAD
[0.4.6]: https://github.com/kingsyy/claudar/compare/v0.4.4...v0.4.6
[0.4.4]: https://github.com/kingsyy/claudar/compare/v0.4.2...v0.4.4
[0.4.2]: https://github.com/kingsyy/claudar/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/kingsyy/claudar/compare/v0.3.0...v0.4.1
[0.3.0]: https://github.com/kingsyy/claudar/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/kingsyy/claudar/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/kingsyy/claudar/releases/tag/v0.1.0
