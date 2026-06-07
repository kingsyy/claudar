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

  let unlistenUsage: UnlistenFn | undefined;
  let unlistenError: UnlistenFn | undefined;
  let tickInterval: ReturnType<typeof setInterval> | undefined;

  let usage = $derived(selected ? usageByInstance[selected] : undefined);
  let error = $derived(selected ? errorByInstance[selected] : undefined);

  async function loadInitialUsage(instance: string) {
    if (usageByInstance[instance]) return;
    try {
      const payload = await invoke<UsagePayload>("get_usage", { instance });
      usageByInstance = { ...usageByInstance, [instance]: payload };
    } catch (e) {
      errorByInstance = { ...errorByInstance, [instance]: String(e) };
    }
  }

  function selectInstance(name: string) {
    selected = name;
    loadInitialUsage(name);
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

  function gaugeColor(pct: number): string {
    if (pct >= 90) return "hsl(0 84% 60%)";
    if (pct >= 70) return "hsl(38 92% 50%)";
    return "hsl(142 71% 45%)";
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
  <h1>Dashboard</h1>

  {#if instances.length > 1}
    <div class="tabs" role="tablist">
      {#each instances as inst (inst.name)}
        <button
          class="tab"
          class:active={selected === inst.name}
          role="tab"
          aria-selected={selected === inst.name}
          onclick={() => selectInstance(inst.name)}
        >
          {inst.name}
        </button>
      {/each}
    </div>
  {/if}

  {#if error}
    <div class="error-banner" role="alert">⚠ {error}</div>
  {/if}

  {#if !usage && !error}
    <div class="loading">
      <div class="spinner" aria-hidden="true"></div>
      <p>Waiting for the first usage update…</p>
    </div>
  {:else if usage}
    <div class="gauges">
      <div class="gauge-card">
        <div
          class="gauge"
          style={`--pct: ${usage.five_hour_pct}; --color: ${gaugeColor(usage.five_hour_pct)}`}
        >
          <span class="gauge-value">{usage.five_hour_pct.toFixed(0)}%</span>
        </div>
        <h2>5-hour usage</h2>
        <p class="sub">Resets in {formatCountdown(usage.resets_at)}</p>
      </div>

      <div class="gauge-card">
        <div
          class="gauge"
          style={`--pct: ${usage.seven_day_pct}; --color: ${gaugeColor(usage.seven_day_pct)}`}
        >
          <span class="gauge-value">{usage.seven_day_pct.toFixed(0)}%</span>
        </div>
        <h2>7-day usage</h2>
        <p class="sub">Resets in {formatCountdown(usage.seven_day_resets_at)}</p>
      </div>
    </div>

    <div class="stat-row">
      <span class="stat-label">Predicted 5-hour burn</span>
      <span class="stat-value">
        {usage.predicted_pct != null ? `${usage.predicted_pct.toFixed(0)}%` : "Not enough data yet"}
      </span>
    </div>
  {/if}
</div>

<style>
  .page {
    padding: 2rem;
    max-width: 720px;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 1rem;
    color: hsl(222.2 84% 4.9%);
  }

  .tabs {
    display: flex;
    gap: 0.25rem;
    border-bottom: 1px solid hsl(var(--border));
    margin-bottom: 1.5rem;
  }

  .tab {
    padding: 0.5rem 1rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
    cursor: pointer;
  }

  .tab:hover {
    color: hsl(var(--foreground));
  }

  .tab.active {
    color: hsl(var(--foreground));
    border-bottom-color: hsl(var(--ring));
    font-weight: 600;
  }

  .error-banner {
    background-color: hsl(0 84% 95%);
    color: hsl(0 70% 40%);
    border: 1px solid hsl(0 84% 80%);
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

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .gauges {
    display: flex;
    gap: 2rem;
    flex-wrap: wrap;
    margin-bottom: 2rem;
  }

  .gauge-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
  }

  .gauge {
    --pct: 0;
    --color: hsl(142 71% 45%);
    width: 9rem;
    height: 9rem;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: conic-gradient(
      var(--color) calc(var(--pct) * 1%),
      hsl(var(--muted)) calc(var(--pct) * 1%)
    );
  }

  .gauge-value {
    background: hsl(var(--background));
    border-radius: 50%;
    width: 6.5rem;
    height: 6.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    font-weight: 600;
    color: hsl(var(--foreground));
  }

  h2 {
    font-size: 0.95rem;
    font-weight: 600;
    margin: 0;
    color: hsl(var(--foreground));
  }

  .sub {
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
    margin: 0;
  }

  .stat-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background-color: hsl(var(--secondary));
    border-radius: var(--radius);
    font-size: 0.875rem;
  }

  .stat-label {
    color: hsl(var(--muted-foreground));
  }

  .stat-value {
    font-weight: 600;
    color: hsl(var(--foreground));
  }
</style>
