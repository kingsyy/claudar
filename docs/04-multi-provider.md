# Multi-provider usage monitoring — research & decisions

**Date:** 2026-08-18 · **Status:** research complete, not implemented · **Scope:** what it takes to
show OpenAI (and later other providers) alongside Claude in one dashboard.

> **Superseded in part on 2026-08-27** by the shipped `openai-web` provider. Corrections, all
> verified against a live account:
>
> - **§6 is wrong about auth.** A plain ChatGPT web login mints a bearer that `/wham/usage`
>   accepts. PKCE (option B) is unnecessary, and so is reading `~/.codex/auth.json` (option A).
>   Scopes were never the constraint: the web token carries no scopes and works.
> - **§6's header warning is moot.** Bearer alone is the minimal set — no `chatgpt-account-id`,
>   no `originator`, none of the rotating `oai-client-*` headers.
> - **New constraint the research missed:** `/api/auth/session` is Cloudflare-blocked over
>   **HTTP/2** and open over **HTTP/1.1**. `/wham/usage` is fine either way. This is why
>   `openai_session.rs` pins `.http1_only()`.
> - **§3.1's window mapping doesn't generalise.** That capture had the 7-day window in
>   `primary_window`; this account has it in `secondary_window` with a 5-hour primary. Select the
>   weekly window **by duration**, never by field name.
> - **§9's sequencing was not followed.** The `History.svelte` split and the `Gauge` model were
>   both skipped: a display-only weekly bar needs neither. They remain the right plan for when
>   history and notifications are wanted.

Everything below is marked **[verified]** (I ran it / read it) or **[inferred]** (reasoned, needs a
live check). Don't build on an inferred line without checking it first.

---

## 1. The question

Claudar monitors one thing: claude.ai's 5-hour and 7-day rolling windows. Can it monitor a paid
OpenAI account too, and become a multi-provider dashboard? OpenAI's limits were assumed to be
"completely differently organized" — that assumption turned out to be mostly wrong.

## 2. Decision: change the model, not the app

**We generalise Claudar's core data model in place. We do not rewrite the app from scratch.**

A ground-up multi-provider rebuild was considered and rejected. The parts of Claudar that are
expensive to rebuild are provider-agnostic already; the part that needs to change is the part
that's mechanical to change.

| Keep — hard-won, provider-agnostic | Replace — mechanical, ~440 refs across 17 files |
|---|---|
| `crypto.rs` — keychain + AES-256-GCM envelope | `UsagePayload` (`monitor.rs:18`) → `Vec<Gauge>` |
| `retry.rs`, `AuthRequiredError`, the fetch fallback chain | `enum LimitType { FiveHour, SevenDay }` (`state.rs:322`) → string keys |
| `state.rs` anti-spam notification state machine | `HistoryRecord`'s 5 fixed fields + JSONL migration |
| `history.rs` storage, `pace.rs` prediction | Thresholds config → per-gauge map |
| Tauri shell, tray main-thread fix, launchd/systemd, wizard | Dashboard / History / Settings rendering N gauges |
| `chrome_auth.rs` — still needed for Claude | — |

That left column is most of ~13k lines and all of the parts that took real debugging (Cloudflare
evasion, the tray `Rc` refcount race, keychain fallback, cookie capture). Throwing it away to
change a struct would be trading a week of typing for a month of rediscovering bugs.

**Scale of the change [verified]:** `grep -rn 'five_hour\|seven_day\|fiveHour\|sevenDay'` over
`crates src src-tauri ui/src` → **440 matches in 17 files**.

**Estimate:** ~3–4 days for the model generalisation, ~0.5 day for the OpenAI provider on top.

## 3. Evidence: OpenAI's limits are *not* differently organised

### 3.1 The live endpoint [verified — captured by hand]

`GET https://chatgpt.com/backend-api/wham/usage` returns:

```json
{
  "user_id": "...", "account_id": "...", "plan_type": "plus",
  "rate_limit": {
    "allowed": true, "limit_reached": false,
    "primary_window": {
      "used_percent": 77, "limit_window_seconds": 604800,
      "reset_after_seconds": 183606, "reset_at": 1787222964
    },
    "secondary_window": null
  },
  "code_review_rate_limit": null,
  "additional_rate_limits": null,
  "credits": { "has_credits": false, "unlimited": false, "balance": "0" },
  "spend_control": { "reached": false, "individual_limit": null }
}
```

Structurally this is Claudar's existing model:

| Claude | OpenAI |
|---|---|
| `five_hour.utilization` + `resets_at` | `primary_window.used_percent` + `reset_at` (604800s = 7d) |
| `seven_day.utilization` + `resets_at` | `secondary_window` (null on this account) |

Percent-of-a-rolling-window with a reset timestamp. Same shape.

### 3.2 But the *count* of limits is dynamic

`secondary_window: null`, `code_review_rate_limit: null`, and **`additional_rate_limits: null`** (a
list) mean OpenAI does not have a fixed set of windows. Confirmed in the Codex binary
[verified — `strings` on `codex` v0.143.0, `/opt/homebrew/Caskroom/codex/0.143.0/`]:

```
GetAccountRateLimitsResponse { rate_limits, rate_limits_by_limit_id, rate_limit_reset_credits }
```

**`rate_limits_by_limit_id` is a map keyed by limit id.** Any fixed-field design breaks the first
time OpenAI adds a limit. Claude's `five_hour`/`seven_day` is the odd one out for being fixed, not
OpenAI for being flexible.

### 3.3 Codex caches usage locally, but it's stale exhaust [verified]

Codex writes a `rate_limits` snapshot into every session rollout log at
`~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`, inside a `TokenCountEvent { info, rate_limits }`:

```json
{ "limit_id": "codex", "limit_name": null,
  "primary": { "used_percent": 50.0, "window_minutes": 10080, "resets_at": 1786279895 },
  "secondary": null, "credits": {...}, "individual_limit": null, "plan_type": "plus" }
```

The newest snapshot on this machine (2026-08-07) said **50%** while the live endpoint said **77%** —
11 days stale, because Codex hadn't run since. Nothing else caches it: `.codex-global-state.json`
and all four SQLite DBs (`logs_2`, `goals_1`, `state_5`, `memories_1`) have no usage data.

**This is the whole argument for Claudar's approach.** A local counter only knows what happened on
this machine, in this tool, since the last run. The server-side endpoint is correct regardless of
which machine or surface burned the quota. Poll the server; never count locally.

### 3.4 Two schemas for the same data [verified]

| Browser wire format (`/wham/usage`) | Codex internal `RateLimitWindow` |
|---|---|
| `primary_window` | `primary` |
| `limit_window_seconds: 604800` | `window_minutes: 10080` |
| `reset_at` | `resets_at` |
| `spend_control.individual_limit` | `individual_limit` (hoisted) |

**[inferred]** The browser capture is the true wire format; the CLI structs are its own internal
representation after a transform, likely surfaced over the local app-server to the Electron UI.
Treat CLI struct names as evidence of *intent*, not as the contract.

### 3.5 There is a server-side token-history endpoint [verified]

```
GetAccountTokenUsageResponse { summary, daily_usage_buckets }
AccountTokenUsageDailyBucket { start_date, tokens }
AccountTokenUsageSummary { ... lifetime ... }
```

Daily buckets plus lifetime totals, server-side. **This is free history backfill** — a new user gets
a populated chart on first launch instead of an empty one, accurate across every machine. Better
than scraping local session logs, which was the earlier idea and is now dropped.

---

## 4. Decision: the `Gauge` model

Replace the fixed pair with a generic, keyed metric list:

```rust
pub struct Gauge {
    pub key: String,            // "claude.five_hour", "openai.codex.primary"
    pub label: String,          // "5-hour window", "Codex weekly"
    pub kind: GaugeKind,        // WindowUtilization | SpendVsBudget | CountVsCap | Balance
    pub value: f64,
    pub max: Option<f64>,       // None = unbounded; render raw value, not a %
    pub unit: Unit,             // Percent | Usd | Messages | Tokens
    pub resets_at: Option<DateTime<Utc>>,
}

trait UsageProvider {
    async fn fetch(&self, creds: &Credential) -> Result<Vec<Gauge>>;
}
```

Downstream consequences:

- `state.rs` keyed by `String` instead of `LimitType`; notification anti-spam state per gauge key.
- `HistoryRecord` becomes `{ polled_at, gauges: [...] }`, with a migration for existing JSONL.
- Thresholds config becomes a map keyed by gauge key. Existing config migrates to
  `claude.five_hour` / `claude.seven_day`.
- `Account { provider, name, credential }` in config; existing instances migrate to
  `provider = "claude-web"`.

**Mixed units are the real UI constraint.** "$41 of $100 this month" and "63% of a 5-hour window"
cannot share a y-axis, and they don't share urgency semantics — one means "budget in 9 days", the
other means "cut off in 40 minutes". Notification copy has to branch on `GaugeKind`, not just
threshold-crossing.

## 5. The UI problem: `History.svelte` is 1908 lines

This is the single biggest implementation risk and deserves its own plan.

| File | Lines | Exposure |
|---|---|---|
| `ui/src/routes/History.svelte` | **1908** | SVG chart hardcoded to two series with a shared 0–100% y-axis |
| `ui/src/routes/Settings.svelte` | **1596** | Threshold editing UI assumes exactly two limits |
| `ui/src/routes/Dashboard.svelte` | 756 | Two gauge cards, hardcoded labels |

`History.svelte` cannot absorb N series of mixed units as-is. A 1900-line single-file component with
a bespoke SVG renderer, 7-day window summaries and its own date maths is where this refactor either
lands or dies.

**Decision: split `History.svelte` before touching the data model, as an independent, shippable
refactor with no behaviour change.** Extract at minimum:

- a chart primitive that takes `series: Series[]` and one unit, rather than 5h/7d fields;
- window-summary computation into `lib/` where it can be unit-tested (it currently can't be);
- per-unit chart *panels*, so % and $ stack as separate charts rather than fighting for an axis.

Doing this first means the model change is a type change against a component that already accepts a
list. Doing it after means debugging two things at once in the worst file in the repo. If the split
turns out to be more than ~a day, that is a signal to reconsider scope, not to push through.

*(Note: `npx svelte-check` is unusable in this repo — it can't find the Svelte plugin in
`vite.config`. Pre-existing. It would be worth fixing before a refactor this size, since it's the
only type safety the UI has.)*

## 6. Codex authentication — three options

The endpoint needs a bearer token. Three ways to get one.

### Option A — read `~/.codex/auth.json`

[verified] The file contains `auth_mode`, `tokens.{id_token, access_token, refresh_token,
account_id}`, `last_refresh`, and an unset `OPENAI_API_KEY`.

- **Pro:** works immediately, zero auth code, refresh token included.
- **Con:** couples to another tool's private file, breaks if Codex changes its format, and does
  nothing for users who don't have Codex installed. Reading another app's credential store is also
  just impolite.
- **Verdict:** ship as an *import-once convenience* in the wizard ("we found a Codex login, use
  it?") that immediately copies the token into Claudar's own keychain entry. Never read it on every
  poll.

### Option B — Claudar runs its own PKCE OAuth flow — **recommended**

[verified] The Codex binary contains the flow's parameters:

```
scopes: openid profile email offline_access api.connectors.read api.connectors.invoke
        code_challenge_method, id_token_add_organizations, codex_cli_simplified_flow, originator
```

`offline_access` is what mints the refresh token.

- **Pro:** independent of Codex being installed; a stable, documented-ish flow; self-refreshing; the
  token is Claudar's own. Much cleaner than the Claude side — no cookies, no Cloudflare, no browser
  needed for polling.
- **Con:** an OAuth + PKCE implementation and a loopback callback listener to write and maintain.
- **Verdict:** the primary path. Store the token via existing `crypto.rs` keychain machinery.
  **Implement refresh from day one** — otherwise the daemon silently goes quiet when the access
  token expires, which is exactly the failure mode a monitoring tool must not have.

### Option C — talk to the local Codex app-server

**[inferred]** `GetAccountRateLimitsResponse` looks like the CLI's own local JSON-RPC surface for
the Electron UI, so Claudar could ask Codex rather than OpenAI and let it own auth entirely.

- **Pro:** cheapest to build; no credential handling at all.
- **Con:** requires Codex installed *and running*; undocumented local protocol.
- **Verdict:** backlog. Possible offline fallback, not the primary.

### Header set — **unverified**

Captured from the browser (values redacted):

```
authorization: Bearer <...>          # required
chatgpt-account-id: <account uuid>   # required — matches auth.json tokens.account_id
Cookie: <...>                        # probably NOT required outside a browser
oai-client-version: prod-1b777b39…   # a git SHA of the web build — DO NOT HARDCODE, rotates
oai-client-build-number: 9432397     # same
oai-device-id / oai-session-id       # telemetry; generate-and-persist a UUID if enforced
X-OpenAI-Target-Path / -Route        # edge routing, duplicates the path — droppable
X-OAI-IS-Client-Observation          # client attestation — if enforced, the browser path is dead
```

**Do not impersonate the browser.** Impersonate the CLI: bearer + `chatgpt-account-id` +
`originator`/`version`. The browser header set contains a build SHA that rotates on every deploy and
an attestation header we can't reproduce; the CLI header set is stable.

**Open question:** the minimal working header set. Untested. A single live call with the local token
settles it — try bearer alone, then + `chatgpt-account-id`, then + `originator`/`version`, and take
the first 200.

## 7. Other open questions

1. **Is `limit_id: "codex"` the same pool as ChatGPT chat?** `wham` is the Codex backend, so the 77%
   may be Codex-only rather than the whole plan. Cheap test: note the current value, use ChatGPT web
   chat heavily, re-fetch. If it moves, shared pool. Until then the dashboard must label it
   **"Codex"**, not "ChatGPT".
2. **Rate-limiting on `/wham/usage`.** Unknown. Claudar's default poll interval must not get the
   account flagged.
3. **Cloudflare.** chatgpt.com sits behind it. A bearer-auth call with a non-browser UA *should*
   pass; if not, `process_response`'s existing challenge detection is reusable.
4. **Naming.** "Claudar" does not survive OpenAI + Gemini + OpenRouter. Rename is a prerequisite for
   marketing this, not for building it.

## 8. Provider backlog, once the trait exists

| Provider | Auth | Gauge kinds | Effort |
|---|---|---|---|
| `claude-web` (existing) | cookies + browser login | WindowUtilization ×2 | done |
| `openai-codex` | PKCE OAuth bearer | WindowUtilization ×N, Balance, SpendVsBudget | 0.5 day |
| `openrouter` | API key | `GET /api/v1/key` returns usage + limit in one call | trivial |
| `anthropic-api` | admin key | SpendVsBudget, Tokens (Usage & Cost API) | small |
| `openai-api` | admin key (`sk-admin-…`) | SpendVsBudget, Tokens (`/v1/organization/costs`) | small |
| `chatgpt-web` | undocumented | message counters behind Settings → Usage Limits | unknown |

Note the split: subscription providers (window %, urgent, cookie/OAuth) and API providers (spend,
non-urgent, admin key) are almost different products sharing a shell. Both fit the `Gauge` model,
but decide deliberately whether the MVP includes spend at all.

## 9. Suggested order

0. **Ship 0.4.5 + code-signing first.** This refactor touches every file; don't carry a release
   through it.
1. **Split `History.svelte`** (§5). No behaviour change, independently shippable.
2. **Introduce `Gauge` + `UsageProvider`; migrate Claude onto it.** Ship with zero user-visible
   change. This is the risky step, de-risked by having exactly one known-good provider.
3. **Add the `openai-codex` provider** (§6 option B, with option A as wizard import).
4. **Backfill history** from `daily_usage_buckets` (§3.5).
5. Decide on spend/API providers as a separate scope call (§8).

## 10. References

- [OpenAI Usage & Cost API cookbook](https://developers.openai.com/cookbook/examples/completions_usage_api)
- [OpenAI costs endpoint reference](https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage/methods/costs)
- [Anthropic Usage & Cost API](https://docs.anthropic.com/en/api/usage-cost-api)
- [ChatGPT per-model and weekly caps](https://tokenkarma.app/chatgpt-usage-limit/)
- Local artifacts inspected: `codex` v0.143.0 binary, `~/.codex/auth.json` (structure only),
  `~/.codex/sessions/**/rollout-*.jsonl`, `~/.codex/.codex-global-state.json`.
