<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type HistoryRecord = {
    polled_at: string;
    five_hour_pct: number;
    five_hour_resets_at: string | null;
    seven_day_pct: number;
    seven_day_resets_at: string | null;
    five_hour_predicted_pct: number | null;
  };

  type InstanceInfo = {
    name: string;
    has_session: boolean;
  };

  const SINCE_DAYS = 7;
  const CHART_WIDTH = 720;
  const CHART_HEIGHT = 220;
  const PADDING = 28;

  let instances = $state<InstanceInfo[]>([]);
  let selected = $state<string | null>(null);
  let recordsByInstance = $state<Record<string, HistoryRecord[]>>({});
  let loading = $state(false);
  let error = $state<string | null>(null);

  let records = $derived(selected ? recordsByInstance[selected] ?? [] : []);

  let stats = $derived.by(() => {
    if (records.length === 0) return null;
    const values = records.map((r) => r.five_hour_pct);
    const sum = values.reduce((acc, v) => acc + v, 0);
    return {
      min: Math.min(...values),
      max: Math.max(...values),
      avg: sum / values.length,
      count: records.length,
    };
  });

  /// Builds an SVG path string for the area under the 5h-usage series.
  let areaPath = $derived.by(() => {
    if (records.length < 2) return null;

    const times = records.map((r) => new Date(r.polled_at).getTime());
    const minTime = Math.min(...times);
    const maxTime = Math.max(...times);
    const timeSpan = maxTime - minTime || 1;

    const innerWidth = CHART_WIDTH - PADDING * 2;
    const innerHeight = CHART_HEIGHT - PADDING * 2;

    const points = records.map((r, i) => {
      const x = PADDING + ((times[i] - minTime) / timeSpan) * innerWidth;
      const y = PADDING + innerHeight - (Math.min(r.five_hour_pct, 100) / 100) * innerHeight;
      return [x, y];
    });

    const line = points.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(2)},${y.toFixed(2)}`).join(" ");
    const baseline = PADDING + innerHeight;
    const [firstX] = points[0];
    const [lastX] = points[points.length - 1];

    return {
      line,
      area: `${line} L${lastX.toFixed(2)},${baseline} L${firstX.toFixed(2)},${baseline} Z`,
    };
  });

  function gridLines(): { y: number; label: string }[] {
    const innerHeight = CHART_HEIGHT - PADDING * 2;
    return [0, 25, 50, 75, 100].map((pct) => ({
      y: PADDING + innerHeight - (pct / 100) * innerHeight,
      label: `${pct}%`,
    }));
  }

  async function loadHistory(instance: string) {
    loading = true;
    error = null;
    try {
      const data = await invoke<HistoryRecord[]>("get_history", {
        instance,
        since_days: SINCE_DAYS,
      });
      recordsByInstance = { ...recordsByInstance, [instance]: data };
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function selectInstance(name: string) {
    selected = name;
    if (!(name in recordsByInstance)) {
      loadHistory(name);
    }
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
  });
</script>

<div class="page">
  <h1>History</h1>
  <p class="subtitle">5-hour usage over the last {SINCE_DAYS} days</p>

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
  {:else if loading}
    <div class="loading">
      <div class="spinner" aria-hidden="true"></div>
      <p>Loading history…</p>
    </div>
  {:else if records.length === 0}
    <div class="empty-state">
      <p>No history recorded yet for this instance.</p>
      <p class="sub">
        History recording can be enabled in your config — once it's on, polls will start
        appearing here.
      </p>
    </div>
  {:else}
    <div class="chart-card">
      <svg
        viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}
        role="img"
        aria-label="5-hour usage percentage over the last 7 days"
      >
        {#each gridLines() as line}
          <line
            x1={PADDING}
            x2={CHART_WIDTH - PADDING}
            y1={line.y}
            y2={line.y}
            class="grid-line"
          />
          <text x={4} y={line.y + 4} class="grid-label">{line.label}</text>
        {/each}

        {#if areaPath}
          <path d={areaPath.area} class="area-fill" />
          <path d={areaPath.line} class="area-line" />
        {/if}
      </svg>
    </div>

    {#if stats}
      <div class="stats-row">
        <div class="stat">
          <span class="stat-label">Min</span>
          <span class="stat-value">{stats.min.toFixed(0)}%</span>
        </div>
        <div class="stat">
          <span class="stat-label">Max</span>
          <span class="stat-value">{stats.max.toFixed(0)}%</span>
        </div>
        <div class="stat">
          <span class="stat-label">Average</span>
          <span class="stat-value">{stats.avg.toFixed(0)}%</span>
        </div>
        <div class="stat">
          <span class="stat-label">Polls recorded</span>
          <span class="stat-value">{stats.count}</span>
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .page {
    padding: 2rem;
    max-width: 800px;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
    color: hsl(222.2 84% 4.9%);
  }

  .subtitle {
    color: hsl(215.4 16.3% 46.9%);
    font-size: 0.875rem;
    margin: 0 0 1.25rem;
  }

  .tabs {
    display: flex;
    gap: 0.25rem;
    margin-bottom: 1.25rem;
    border-bottom: 1px solid hsl(214.3 31.8% 91.4%);
  }

  .tab {
    padding: 0.5rem 1rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 0.875rem;
    color: hsl(215.4 16.3% 46.9%);
    cursor: pointer;
  }

  .tab.active {
    color: hsl(222.2 84% 4.9%);
    border-bottom-color: hsl(222.2 84% 4.9%);
    font-weight: 600;
  }

  .error-banner {
    padding: 0.75rem 1rem;
    background-color: hsl(0 84% 95%);
    color: hsl(0 70% 40%);
    border-radius: var(--radius);
    font-size: 0.875rem;
  }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding: 3rem 0;
    color: hsl(215.4 16.3% 46.9%);
    font-size: 0.875rem;
  }

  .spinner {
    width: 28px;
    height: 28px;
    border: 3px solid hsl(214.3 31.8% 91.4%);
    border-top-color: hsl(222.2 84% 4.9%);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .empty-state {
    padding: 3rem 1rem;
    text-align: center;
    color: hsl(215.4 16.3% 46.9%);
  }

  .empty-state p {
    margin: 0 0 0.5rem;
    font-size: 0.9rem;
  }

  .empty-state .sub {
    font-size: 0.8rem;
    max-width: 420px;
    margin: 0 auto;
  }

  .chart-card {
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    padding: 1rem;
    background-color: hsl(0 0% 100%);
  }

  svg {
    width: 100%;
    height: auto;
    display: block;
  }

  .grid-line {
    stroke: hsl(214.3 31.8% 91.4%);
    stroke-width: 1;
  }

  .grid-label {
    font-size: 9px;
    fill: hsl(215.4 16.3% 56%);
  }

  .area-fill {
    fill: hsl(222.2 84% 60% / 0.15);
  }

  .area-line {
    fill: none;
    stroke: hsl(222.2 84% 50%);
    stroke-width: 2;
  }

  .stats-row {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1rem;
    margin-top: 1.25rem;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.85rem 1rem;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
  }

  .stat-label {
    font-size: 0.75rem;
    color: hsl(215.4 16.3% 46.9%);
  }

  .stat-value {
    font-size: 1.1rem;
    font-weight: 600;
    color: hsl(222.2 84% 4.9%);
  }
</style>
