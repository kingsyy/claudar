<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  let selected = $state<string | null>(null);
  let usageByInstance = $state<Record<string, UsagePayload>>({});
  let errorByInstance = $state<Record<string, string>>({});
  let now = $state(Date.now());
  let lastUpdateByInstance = $state<Record<string, number>>({});

  let unlistenUsage: UnlistenFn | undefined;
  let unlistenError: UnlistenFn | undefined;
  let tickInterval: ReturnType<typeof setInterval> | undefined;
  let refreshing = $state(false);

  let usage = $derived(selected ? usageByInstance[selected] : undefined);
  let error = $derived(selected ? errorByInstance[selected] : undefined);
  let lastUpdate = $derived(selected ? lastUpdateByInstance[selected] : undefined);

  async function fetchUsage(instance: string) {
    try {
      const payload = await invoke<UsagePayload>("get_usage", { instance });
      usageByInstance = { ...usageByInstance, [instance]: payload };
      lastUpdateByInstance = { ...lastUpdateByInstance, [instance]: Date.now() };
      if (instance in errorByInstance) {
        const next = { ...errorByInstance };
        delete next[instance];
        errorByInstance = next;
      }
    } catch (e) {
      errorByInstance = { ...errorByInstance, [instance]: String(e) };
    }
  }

  async function loadInitialUsage(instance: string) {
    if (usageByInstance[instance]) return;
    await fetchUsage(instance);
  }

  async function refresh() {
    if (!selected || refreshing) return;
    refreshing = true;
    try {
      await fetchUsage(selected);
    } finally {
      refreshing = false;
    }
  }

  function selectInstance(name: string) {
    selected = name;
    const inst = instances.find(i => i.name === name);
    if (inst?.has_session) {
      loadInitialUsage(name);
    } else {
      errorByInstance = { ...errorByInstance, [name]: "No session configured — open Settings to sign in" };
    }
  }

  function formatCountdown(iso: string | null | undefined): string {
    if (!iso) return "—";
    const target = new Date(iso).getTime();
    const diffMs = target - now;
    if (diffMs <= 0) return "resetting…";
    const minutes = Math.floor(diffMs / 60_000);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    if (days > 0) return `${days}d ${hours % 24}h`;
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    return `${minutes}m`;
  }

  function formatAbsoluteTime(iso: string | null | undefined): string {
    if (!iso) return "";
    const d = new Date(iso);
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  const FIVE_HOUR_MS = 5 * 60 * 60 * 1000;
  const SEVEN_DAY_MS = 7 * 24 * 60 * 60 * 1000;

  function timeElapsedPct(iso: string | null | undefined, windowMs: number): number {
    if (!iso) return 0;
    const remaining = new Date(iso).getTime() - now;
    if (remaining <= 0) return 100;
    const elapsed = windowMs - remaining;
    if (elapsed <= 0) return 0;
    return Math.min(100, (elapsed / windowMs) * 100);
  }

  // Mirrors the Rust pace::calculate_pace_info logic (±10% dead band)
  function paceInfo(usagePct: number, timePct: number): { label: string; kind: "over" | "under" | "on" } {
    const diff = usagePct - timePct;
    if (diff > 10) return { label: `${diff.toFixed(1)}% over pace`, kind: "over" };
    if (diff < -10) return { label: `${Math.abs(diff).toFixed(1)}% under pace`, kind: "under" };
    return { label: "On pace", kind: "on" };
  }

  function statusInfo(pct: number): { label: string; kind: "ok" | "elevated" | "warning" | "critical" } {
    if (pct >= 90) return { label: "Critical", kind: "critical" };
    if (pct >= 70) return { label: "Warning", kind: "warning" };
    if (pct >= 50) return { label: "Elevated", kind: "elevated" };
    return { label: "OK", kind: "ok" };
  }

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
      if (instances.length > 0 && selected === null) {
        selectInstance(instances[0].name);
      }
    } catch (e) {
      console.error("get_instances failed", e);
    }

    unlistenUsage = await listen<UsagePayload>("usage-update", (event) => {
      const payload = event.payload;
      usageByInstance = { ...usageByInstance, [payload.instance]: payload };
      lastUpdateByInstance = { ...lastUpdateByInstance, [payload.instance]: Date.now() };
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
  });
</script>

<div class="page">
  <div class="page-header">
    <div>
      <h1>Dashboard</h1>
      <p class="subtitle">Live usage across your Claude.ai limits.</p>
    </div>
    <div class="controls">
      {#if instances.length > 1}
        <div class="tabs" role="tablist">
          {#each instances as inst (inst.name)}
            <button
              class="tab"
              class:active={selected === inst.name}
              role="tab"
              aria-selected={selected === inst.name}
              onclick={() => selectInstance(inst.name)}>{inst.name}</button
            >
          {/each}
        </div>
      {/if}
      {#if usage}
        <span class="updated">Updated {formatTimeSince(lastUpdate)}</span>
      {/if}
      <button class="refresh-btn" class:spinning={refreshing} onclick={refresh} disabled={refreshing} aria-label="Refresh">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
          <path d="M13.5 8A5.5 5.5 0 1 1 10 3.07"/>
          <polyline points="10 1 10 4 13 4"/>
        </svg>
      </button>
    </div>
  </div>

  {#if error}
    <div class="error-banner" role="alert">⚠ {error}</div>
  {/if}

  {#if !usage && !error}
    <div class="loading">
      <div class="spinner" aria-hidden="true"></div>
      <p>Waiting for the first usage update…</p>
    </div>
  {:else if usage}
    {@const fiveTimePct = timeElapsedPct(usage.resets_at, FIVE_HOUR_MS)}
    {@const sevenTimePct = timeElapsedPct(usage.seven_day_resets_at, SEVEN_DAY_MS)}
    {@const fivePace = paceInfo(usage.five_hour_pct, fiveTimePct)}
    {@const sevenPace = paceInfo(usage.seven_day_pct, sevenTimePct)}
    {@const fiveStatus = statusInfo(usage.five_hour_pct)}
    {@const sevenStatus = statusInfo(usage.seven_day_pct)}

    <div class="limit-cards">
      <!-- 5-hour card -->
      <div class="limit-card">
        <div class="card-header">
          <span class="card-title">5-Hour Limit</span>
          <span class="status-badge status-{fiveStatus.kind}">{fiveStatus.label}</span>
          <span class="pace-badge pace-{fivePace.kind}">{fivePace.label}</span>
        </div>

        <div class="bars">
          <div class="bar-row">
            <span class="bar-icon" title="Time elapsed">⏱</span>
            <div class="bar-track">
              <div class="bar-fill bar-time" style="width: {fiveTimePct}%"></div>
            </div>
            <span class="bar-pct muted">{fiveTimePct.toFixed(1)}%</span>
          </div>
          <div class="bar-row">
            <span class="bar-icon" title="Tokens used">💬</span>
            <div class="bar-track">
              <div class="bar-fill bar-usage bar-usage-{fiveStatus.kind}" style="width: {usage.five_hour_pct}%"></div>
            </div>
            <span class="bar-pct">{usage.five_hour_pct.toFixed(1)}%</span>
          </div>
        </div>

        <div class="card-footer">
          Resets in <strong>{formatCountdown(usage.resets_at)}</strong>
          {#if usage.resets_at}
            <span class="muted">· at {formatAbsoluteTime(usage.resets_at)}</span>
          {/if}
          {#if usage.predicted_pct != null}
            <span class="muted">· predicted peak {usage.predicted_pct.toFixed(0)}%</span>
          {/if}
        </div>
      </div>

      <!-- 7-day card -->
      <div class="limit-card">
        <div class="card-header">
          <span class="card-title">7-Day Limit</span>
          <span class="status-badge status-{sevenStatus.kind}">{sevenStatus.label}</span>
          <span class="pace-badge pace-{sevenPace.kind}">{sevenPace.label}</span>
        </div>

        <div class="bars">
          <div class="bar-row">
            <span class="bar-icon" title="Time elapsed">⏱</span>
            <div class="bar-track">
              <div class="bar-fill bar-time" style="width: {sevenTimePct}%"></div>
            </div>
            <span class="bar-pct muted">{sevenTimePct.toFixed(1)}%</span>
          </div>
          <div class="bar-row">
            <span class="bar-icon" title="Tokens used">💬</span>
            <div class="bar-track">
              <div class="bar-fill bar-usage bar-usage-{sevenStatus.kind}" style="width: {usage.seven_day_pct}%"></div>
            </div>
            <span class="bar-pct">{usage.seven_day_pct.toFixed(1)}%</span>
          </div>
        </div>

        <div class="card-footer">
          Resets in <strong>{formatCountdown(usage.seven_day_resets_at)}</strong>
          {#if usage.seven_day_resets_at}
            <span class="muted">· at {formatAbsoluteTime(usage.seven_day_resets_at)}</span>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .page {
    padding: 2rem;
    max-width: 680px;
  }

  .page-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1.25rem;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .updated {
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
    white-space: nowrap;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.2rem;
    color: hsl(var(--foreground));
  }

  .subtitle {
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
    margin: 0;
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
    border-color: hsl(var(--border));
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

  /* Instance tabs (segmented, matches History) */
  .tabs {
    display: flex;
    gap: 0.25rem;
    border: 1px solid hsl(var(--border));
    border-radius: 6px;
    padding: 2px;
    background: hsl(var(--muted));
  }

  .tab {
    padding: 0.3rem 0.75rem;
    background: none;
    border: none;
    border-radius: 4px;
    font-size: 0.8125rem;
    color: hsl(var(--muted-foreground));
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }

  .tab:hover { color: hsl(var(--foreground)); }

  .tab.active {
    background: hsl(var(--card));
    color: hsl(var(--foreground));
    font-weight: 600;
    box-shadow: 0 1px 3px hsl(222.2 84% 4.9% / 0.08);
  }

  /* Error / loading */
  .error-banner {
    background-color: hsl(var(--danger-bg));
    color: hsl(var(--danger-strong));
    border: 1px solid hsl(var(--danger-border));
    border-radius: var(--radius);
    padding: 0.75rem 1rem;
    font-size: 0.875rem;
    margin-bottom: 1.5rem;
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

  .spinner {
    width: 2rem;
    height: 2rem;
    border: 3px solid hsl(var(--border));
    border-top-color: hsl(var(--ring));
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* Cards */
  .limit-cards {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1.25rem;
  }

  .limit-card {
    background: hsl(var(--card, var(--background)));
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  /* Card header */
  .card-header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .card-title {
    font-weight: 600;
    font-size: 0.95rem;
    color: hsl(var(--foreground));
    flex: 1;
  }

  /* Status badge */
  .status-badge {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.15rem 0.5rem;
    border-radius: 99px;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  .status-ok       { background: hsl(var(--success-bg)); color: hsl(var(--success-strong)); }
  .status-elevated { background: hsl(var(--warning-bg)); color: hsl(var(--warning-strong)); }
  .status-warning  { background: hsl(var(--warning-bg)); color: hsl(var(--warning-strong)); }
  .status-critical { background: hsl(var(--danger-bg));  color: hsl(var(--danger-strong));  }

  /* Pace badge */
  .pace-badge {
    font-size: 0.78rem;
    color: hsl(var(--muted-foreground));
  }

  .pace-over  { color: hsl(var(--warning-strong)); }
  .pace-under { color: hsl(var(--success-strong)); }
  .pace-on    { color: hsl(var(--muted-foreground)); }

  /* Progress bars */
  .bars {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .bar-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .bar-icon {
    width: 1.25rem;
    text-align: center;
    font-size: 0.85rem;
    flex-shrink: 0;
  }

  .bar-track {
    flex: 1;
    height: 8px;
    background: hsl(var(--muted));
    border-radius: 99px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 99px;
    transition: width 0.4s ease;
  }

  .bar-time {
    background: hsl(var(--muted-foreground));
  }

  .bar-usage-ok       { background: hsl(var(--success)); }
  .bar-usage-elevated { background: hsl(var(--warning)); }
  .bar-usage-warning  { background: hsl(var(--warning)); }
  .bar-usage-critical { background: hsl(var(--danger)); }

  .bar-pct {
    font-size: 0.78rem;
    font-weight: 500;
    width: 3.5rem;
    text-align: right;
    flex-shrink: 0;
    color: hsl(var(--foreground));
  }

  /* Card footer */
  .card-footer {
    font-size: 0.8rem;
    color: hsl(var(--foreground));
  }

  .muted {
    color: hsl(var(--muted-foreground));
  }
</style>
