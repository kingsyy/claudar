<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { formatResetTimestamp, paceVerdict, projectedPeak } from "$lib/dashboard";

  type UsagePayload = {
    instance: string;
    five_hour_pct: number;
    seven_day_pct: number;
    resets_at: string | null;
    seven_day_resets_at: string | null;
    predicted_pct: number | null;
  };

  type InstanceInfo = {
    name: string;
    has_session: boolean;
  };

  let instances = $state<InstanceInfo[]>([]);
  let showPaceDelta = $state(false);
  let usageByInstance = $state<Record<string, UsagePayload>>({});
  let errorByInstance = $state<Record<string, string>>({});
  let now = $state(Date.now());
  let lastUpdateByInstance = $state<Record<string, number>>({});
  // Bumped on every fresh payload — drives the "just updated" shimmer.
  let refreshSeqByInstance = $state<Record<string, number>>({});

  let unlistenUsage: UnlistenFn | undefined;
  let unlistenError: UnlistenFn | undefined;
  let tickInterval: ReturnType<typeof setInterval> | undefined;
  let copyFeedbackTimeout: ReturnType<typeof setTimeout> | undefined;
  let refreshing = $state(false);
  let copiedReset = $state<string | null>(null);

  // Newest update across every instance, for the single header timestamp.
  let lastUpdate = $derived(
    Object.values(lastUpdateByInstance).reduce((a, b) => Math.max(a, b), 0) || undefined,
  );

  function markUpdated(instance: string) {
    lastUpdateByInstance = { ...lastUpdateByInstance, [instance]: Date.now() };
    refreshSeqByInstance = {
      ...refreshSeqByInstance,
      [instance]: (refreshSeqByInstance[instance] ?? 0) + 1,
    };
  }

  async function fetchUsage(instance: string) {
    try {
      const payload = await invoke<UsagePayload>("get_usage", { instance });
      usageByInstance = { ...usageByInstance, [instance]: payload };
      markUpdated(instance);
      if (instance in errorByInstance) {
        const next = { ...errorByInstance };
        delete next[instance];
        errorByInstance = next;
      }
    } catch (e) {
      errorByInstance = { ...errorByInstance, [instance]: String(e) };
    }
  }

  function loadInstance(inst: InstanceInfo) {
    if (inst.has_session) {
      if (!usageByInstance[inst.name]) fetchUsage(inst.name);
    } else {
      errorByInstance = {
        ...errorByInstance,
        [inst.name]: "No session configured — open Settings to sign in",
      };
    }
  }

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    try {
      await Promise.all(
        instances.filter((i) => i.has_session).map((i) => fetchUsage(i.name)),
      );
    } finally {
      refreshing = false;
    }
  }

  const FIVE_HOUR_MS = 5 * 60 * 60 * 1000;
  const SEVEN_DAY_MS = 7 * 24 * 60 * 60 * 1000;

  const clampPct = (n: number) => Math.max(0, Math.min(100, n));

  function formatCountdown(iso: string | null | undefined): string {
    if (!iso) return "—";
    const diffMs = new Date(iso).getTime() - now;
    if (diffMs <= 0) return "resetting…";
    const minutes = Math.floor(diffMs / 60_000);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    if (days > 0) return `${days}d ${hours % 24}h`;
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    return `${minutes}m`;
  }

  async function copyResetTimestamp(iso: string, key: string) {
    const text = formatResetTimestamp(iso);
    await navigator.clipboard.writeText(text);
    copiedReset = key;
    if (copyFeedbackTimeout) clearTimeout(copyFeedbackTimeout);
    copyFeedbackTimeout = setTimeout(() => {
      copiedReset = null;
    }, 1800);
  }

  function timeElapsedPct(iso: string | null | undefined, windowMs: number): number {
    if (!iso) return 0;
    const remaining = new Date(iso).getTime() - now;
    if (remaining <= 0) return 100;
    const elapsed = windowMs - remaining;
    if (elapsed <= 0) return 0;
    return Math.min(100, (elapsed / windowMs) * 100);
  }

  // Colour distinguishes acceptable pace, a moderate overage, and a large overage.

  function formatTimeSince(timestamp: number | undefined): string {
    if (!timestamp) return "Never";
    const diffMs = now - timestamp;
    if (diffMs < 60_000) return "Just now";
    const minutes = Math.floor(diffMs / 60_000);
    if (minutes === 1) return "1 minute ago";
    if (minutes < 60) return `${minutes} minutes ago`;
    const hours = Math.floor(minutes / 60);
    if (hours === 1) return "1 hour ago";
    return `${hours} hours ago`;
  }

  onMount(async () => {
    try {
      instances = await invoke<InstanceInfo[]>("get_instances");
      for (const inst of instances) loadInstance(inst);
    } catch (e) {
      console.error("get_instances failed", e);
    }

    try {
      const cfg = await invoke<{ general: { show_pace_delta: boolean } }>("get_config", {});
      showPaceDelta = cfg.general.show_pace_delta;
    } catch (e) {
      console.error("get_config failed", e);
    }

    unlistenUsage = await listen<UsagePayload>("usage-update", (event) => {
      const payload = event.payload;
      usageByInstance = { ...usageByInstance, [payload.instance]: payload };
      markUpdated(payload.instance);
      if (payload.instance in errorByInstance) {
        const next = { ...errorByInstance };
        delete next[payload.instance];
        errorByInstance = next;
      }
    });

    unlistenError = await listen<{ instance: string; message: string }>(
      "monitor-error",
      (event) => {
        const { instance, message } = event.payload;
        errorByInstance = { ...errorByInstance, [instance]: message };
      },
    );

    tickInterval = setInterval(() => {
      now = Date.now();
    }, 30_000);
  });

  onDestroy(() => {
    unlistenUsage?.();
    unlistenError?.();
    if (tickInterval) clearInterval(tickInterval);
    if (copyFeedbackTimeout) clearTimeout(copyFeedbackTimeout);
  });
</script>

{#snippet limit(label: string, pct: number, resetsAt: string | null, windowMs: number, predicted: number | null, seq: number, copyKey: string)}
  {@const timePct = timeElapsedPct(resetsAt, windowMs)}
  {@const peak = projectedPeak(pct, timePct, predicted)}
  {@const kind = paceVerdict(pct, peak)}
  {@const over = peak > 100}
  {@const capped = pct >= 99}
  {@const fillW = clampPct(pct)}
  {@const peakEnd = clampPct(peak)}
  {@const ghostW = Math.max(0, peakEnd - fillW)}
  {@const nowPos = Math.max(7, Math.min(93, fillW))}
  {@const peakPos = Math.max(7, Math.min(95, peakEnd))}
  {@const gapOk = peakPos - nowPos >= 13}
  {@const showPeak = !capped && peak - pct >= 3 && (over || gapOk)}
  {@const showNow = !capped && (!over || peakPos - nowPos >= 13)}
  {@const delta = Math.round(pct - timePct)}
  {@const showDelta = showPaceDelta && !(pct <= 0 && timePct <= 0)}
  {@const deltaAtEnd = fillW < 3}
  {@const deltaInside = fillW >= 12}
  {@const deltaTone = delta > 0 ? kind : "ok"}
  {@const timePos = clampPct(timePct)}
  {@const showTimeTick = timePct > 0.5 && timePct < 99.5}
  <div class="limit">
    <div class="limit-head">
      <span class="win">{label}</span>
      <span class="reset">
        <span class="reset-word">resets</span>
        {#if resetsAt}
          <span class="reset-control">
            <button
              type="button"
              class="reset-val"
              onclick={() => copyResetTimestamp(resetsAt, copyKey)}
              aria-label={`Reset in ${formatCountdown(resetsAt)}. Copy exact expiry time.`}
            >{formatCountdown(resetsAt)}</button>
            <span class="reset-tooltip" role="status">
              <span class="reset-tooltip-label">{copiedReset === copyKey ? "Copied" : "Expires"}</span>
              <time datetime={resetsAt}>{formatResetTimestamp(resetsAt)}</time>
              <span class="reset-tooltip-hint">
                {copiedReset === copyKey ? "Copied to clipboard" : "Click countdown to copy"}
              </span>
            </span>
          </span>
        {:else}
          <span class="reset-val reset-unavailable">—</span>
        {/if}
      </span>
    </div>

    <div class="bar-wrap">
      {#if capped}
        <div class="mark mark-now tone-crit" style="left: {nowPos}%">100%</div>
      {:else}
        {#if showNow}
          <div class="mark mark-now" style="left: {nowPos}%">{pct.toFixed(0)}%</div>
        {/if}
        {#if showPeak}
          <div class="mark mark-peak tone-{kind}" style="left: {peakPos}%">{Math.round(peak)}%</div>
        {/if}
      {/if}

      <div class="track">
        <div class="bar-fill bar-{kind}" style="width: {fillW}%"></div>
        {#if showTimeTick}
          <div class="time-tick" style="left: {timePos}%" title="{timePct.toFixed(0)}% of window elapsed"></div>
        {/if}
        {#if ghostW > 0.4}
          <div class="bar-ghost tone-{kind}" style="left: {fillW}%; width: {ghostW}%"></div>
        {/if}
        {#if over}<div class="overflow-nub tone-{kind}"></div>{/if}
        {#if showDelta}
          <div
            class="delta tone-{deltaTone}"
            class:delta-end={deltaAtEnd}
            class:delta-inside={deltaInside && !deltaAtEnd}
            style={deltaAtEnd ? "" : `left: ${fillW}%`}
          >
            {delta > 0 ? `+${delta}` : delta < 0 ? `−${Math.abs(delta)}` : "0"}%
          </div>
        {/if}
        {#key seq}
          <div class="sweep" aria-hidden="true"></div>
        {/key}
      </div>
    </div>
  </div>
{/snippet}

<div class="page">
  <div class="page-header">
    {#if lastUpdate}
      <span class="updated">Updated {formatTimeSince(lastUpdate)}</span>
    {:else}
      <span class="updated">Loading…</span>
    {/if}
    <button class="refresh-btn" class:spinning={refreshing} onclick={refresh} disabled={refreshing} aria-label="Refresh all">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
        <path d="M13.5 8A5.5 5.5 0 1 1 10 3.07"/>
        <polyline points="10 1 10 4 13 4"/>
      </svg>
    </button>
  </div>

  {#if instances.length === 0}
    <div class="loading">
      <div class="spinner" aria-hidden="true"></div>
      <p>Loading accounts…</p>
    </div>
  {/if}

  {#each instances as inst (inst.name)}
    {@const usage = usageByInstance[inst.name]}
    {@const error = errorByInstance[inst.name]}
    {@const seq = refreshSeqByInstance[inst.name] ?? 0}
    <section class="instance">
      {#if instances.length > 1}
        <h2 class="instance-name">{inst.name}</h2>
      {/if}

      {#if error}
        <div class="error-banner" role="alert">⚠ {error}</div>
      {:else if !usage}
        <div class="instance-loading">
          <div class="spinner small" aria-hidden="true"></div>
          <span>Waiting for first update…</span>
        </div>
      {:else}
        {@render limit("5-hour", usage.five_hour_pct, usage.resets_at, FIVE_HOUR_MS, usage.predicted_pct, seq, `${inst.name}-5h`)}
        {@render limit("7-day", usage.seven_day_pct, usage.seven_day_resets_at, SEVEN_DAY_MS, null, seq, `${inst.name}-7d`)}
      {/if}
    </section>
  {/each}
</div>

<style>
  .page {
    padding: 0.85rem;
  }

  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 0.85rem;
  }

  .updated {
    font-size: 0.75rem;
    color: hsl(var(--muted-foreground));
    white-space: nowrap;
  }

  .refresh-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    padding: 0;
    background: none;
    border: 1px solid hsl(var(--border));
    border-radius: 6px;
    color: hsl(var(--muted-foreground));
    cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
    flex-shrink: 0;
  }

  .refresh-btn:hover:not(:disabled) {
    background: hsl(var(--muted));
    color: hsl(var(--foreground));
  }

  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .refresh-btn svg {
    width: 14px;
    height: 14px;
  }

  .refresh-btn.spinning svg {
    animation: spin 0.7s linear infinite;
  }

  /* Error / loading */
  .error-banner {
    background-color: hsl(var(--danger-bg));
    color: hsl(var(--danger-strong));
    border: 1px solid hsl(var(--danger-border));
    border-radius: var(--radius);
    padding: 0.5rem 0.7rem;
    font-size: 0.8rem;
  }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    padding: 3rem 0;
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
  }

  .instance-loading {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0;
    color: hsl(var(--muted-foreground));
    font-size: 0.8rem;
  }

  .spinner {
    width: 2rem;
    height: 2rem;
    border: 3px solid hsl(var(--border));
    border-top-color: hsl(var(--ring));
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .spinner.small {
    width: 1rem;
    height: 1rem;
    border-width: 2px;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Per-instance section ───────────────────────────────────────── */
  .instance {
    background: hsl(var(--card, var(--background)));
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    padding: 0.85rem 0.95rem;
    margin-bottom: 0.7rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .instance-name {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: hsl(var(--muted-foreground));
    margin: 0 0 -0.4rem;
  }

  /* ── One limit ── */
  .limit {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .limit-head {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .win {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: hsl(var(--muted-foreground));
  }

  .reset {
    margin-left: auto;
    font-size: 0.72rem;
    display: inline-flex;
    align-items: baseline;
    gap: 0.2rem;
  }

  .reset-word {
    color: hsl(var(--muted-foreground));
    opacity: 0.7;
  }

  .reset-val {
    font-weight: 700;
    color: hsl(var(--foreground));
    font-variant-numeric: tabular-nums;
  }

  button.reset-val {
    appearance: none;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-weight: 700;
    color: hsl(var(--foreground));
    cursor: copy;
    border-radius: 3px;
  }

  button.reset-val:hover,
  button.reset-val:focus-visible {
    color: hsl(var(--ring));
    outline: none;
  }

  button.reset-val:focus-visible {
    box-shadow: 0 0 0 2px hsl(var(--ring) / 0.35);
  }

  .reset-control {
    position: relative;
    display: inline-flex;
  }

  .reset-tooltip {
    position: absolute;
    z-index: 20;
    right: 0;
    bottom: calc(100% + 0.5rem);
    width: max-content;
    max-width: min(19rem, calc(100vw - 2rem));
    padding: 0.55rem 0.65rem;
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
    border: 1px solid hsl(var(--border));
    border-radius: 6px;
    background: hsl(var(--popover, var(--card, var(--background))));
    color: hsl(var(--foreground));
    box-shadow: 0 8px 24px hsl(0 0% 0% / 0.18);
    opacity: 0;
    visibility: hidden;
    transform: translateY(3px);
    pointer-events: none;
    transition: opacity 0.12s ease, transform 0.12s ease, visibility 0.12s;
  }

  .reset-control:hover .reset-tooltip,
  .reset-control:focus-within .reset-tooltip {
    opacity: 1;
    visibility: visible;
    transform: translateY(0);
  }

  .reset-tooltip-label {
    color: hsl(var(--muted-foreground));
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .reset-tooltip time {
    font-size: 0.76rem;
    font-weight: 650;
    white-space: nowrap;
  }

  .reset-tooltip-hint {
    color: hsl(var(--muted-foreground));
    font-size: 0.66rem;
  }

  .reset-unavailable {
    margin-left: 0;
  }

  /* ── Bar + its anchored number marks ── */
  .bar-wrap {
    position: relative;
    padding-top: 1.2rem; /* room for the marks that sit above the bar */
  }

  .mark {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    font-size: 0.72rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    transition: left 0.45s ease, color 0.3s ease;
  }

  /* little caret tying the number to its spot on the bar */
  .mark::after {
    content: "";
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    margin-top: 1px;
    border-left: 3px solid transparent;
    border-right: 3px solid transparent;
    border-top: 3px solid currentColor;
  }

  .mark-now { color: hsl(var(--foreground)); }
  .mark-now.tone-crit { color: hsl(var(--danger-strong)); }

  .mark-peak { font-weight: 600; color: hsl(var(--muted-foreground)); }
  .mark-peak.tone-warn { color: hsl(var(--warning-strong)); }
  .mark-peak.tone-crit { color: hsl(var(--danger-strong)); }

  /* ── The bar is the hero: tall, rounded, colour = health ── */
  .track {
    position: relative;
    height: 16px;
    background: hsl(var(--muted));
    border-radius: 99px;
    box-shadow: inset 0 1px 2px hsl(0 0% 0% / 0.10);
  }

  .bar-fill {
    position: absolute;
    inset: 0 auto 0 0;
    height: 100%;
    box-sizing: border-box;
    border-radius: 99px;
    transition: width 0.45s ease, background 0.3s ease, border-color 0.3s ease;
  }

  /* Pill styling: pastel fill, stronger tone reserved for text/border — same
     pairing used by badges elsewhere (error-banner, etc). */
  .bar-ok   { background: hsl(var(--success-bg)); border: 1px solid hsl(var(--success-border)); }
  .bar-warn { background: hsl(var(--warning-bg)); border: 1px solid hsl(var(--warning-border)); }
  .bar-crit { background: hsl(var(--danger-bg));  border: 1px solid hsl(var(--danger-border)); }

  /* ── Pace delta (usage% − time-elapsed%): anchored to the fill's own edge so
     it tracks the bar without measuring text width. ── */
  .delta {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    font-size: 0.66rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    pointer-events: none;
    transition: left 0.45s ease, color 0.3s ease;
  }

  /* Enough room in the fill: sit inside it, inset from its trailing edge. */
  .delta.delta-inside {
    transform: translate(calc(-100% - 6px), -50%);
  }

  /* Not enough room: sit just outside the fill's trailing edge. */
  .delta:not(.delta-inside):not(.delta-end) {
    transform: translate(6px, -50%);
  }

  /* Fill is ~0%: nothing to anchor to, pin to the end of the track instead. */
  .delta.delta-end {
    left: auto;
    right: 6px;
  }

  .delta.tone-ok   { color: hsl(var(--success-strong)); }
  .delta.tone-warn { color: hsl(var(--warning-strong)); }
  .delta.tone-crit { color: hsl(var(--danger-strong)); }

  /* Time-elapsed tick: where usage "should" be if it tracked the window evenly. */
  .time-tick {
    position: absolute;
    top: -3px;
    bottom: -3px;
    width: 2px;
    background: hsl(var(--foreground) / 0.55);
    transform: translateX(-50%);
    transition: left 0.45s ease;
    pointer-events: none;
  }

  /* Projected peak: hatched extension beyond the current fill. */
  .bar-ghost {
    position: absolute;
    top: 0;
    bottom: 0;
    border-radius: 0 99px 99px 0;
    background-color: hsl(var(--foreground) / 0.05);
    background-image: repeating-linear-gradient(
      135deg,
      hsl(var(--foreground) / 0.30) 0 3px,
      transparent 3px 7px
    );
    transition: left 0.45s ease, width 0.45s ease;
  }

  .bar-ghost.tone-ok {
    background-image: repeating-linear-gradient(
      135deg,
      hsl(var(--success) / 0.55) 0 3px,
      transparent 3px 7px
    );
  }

  .bar-ghost.tone-warn {
    background-image: repeating-linear-gradient(
      135deg,
      hsl(var(--warning) / 0.55) 0 3px,
      transparent 3px 7px
    );
  }

  .bar-ghost.tone-crit {
    background-image: repeating-linear-gradient(
      135deg,
      hsl(var(--danger) / 0.55) 0 3px,
      transparent 3px 7px
    );
  }

  /* Projected to blow past the cap — arrow poking off the end. */
  .overflow-nub {
    position: absolute;
    right: -9px;
    top: 50%;
    transform: translateY(-50%);
    width: 0;
    height: 0;
    border-top: 5px solid transparent;
    border-bottom: 5px solid transparent;
    border-left: 7px solid transparent;
  }

  .overflow-nub.tone-ok { border-left-color: hsl(var(--success)); }
  .overflow-nub.tone-warn { border-left-color: hsl(var(--warning)); }
  .overflow-nub.tone-crit { border-left-color: hsl(var(--danger)); }

  /* ── "Just updated" shimmer: one light pass across the bar ── */
  .sweep {
    position: absolute;
    inset: 0;
    border-radius: 99px;
    overflow: hidden;
    pointer-events: none;
  }

  .sweep::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 45%;
    background: linear-gradient(
      90deg,
      transparent,
      hsl(0 0% 100% / 0.45),
      transparent
    );
    transform: translateX(-140%);
    animation: sweep 0.65s ease-out;
  }

  @keyframes sweep {
    to { transform: translateX(360%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .sweep::after { animation: none; }
    .bar-fill, .bar-ghost, .mark, .delta { transition: none; }
  }
</style>
