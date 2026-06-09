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

  type WindowRow = {
    window_start: string;
    window_end: string;
    peak_five_hour_pct: number;
    hit_100: boolean;
    time_to_100_mins: number | null;
    seven_day_pct_at_start: number;
    seven_day_pct_at_end: number;
    seven_day_delta: number;
  };

  type TooltipState = {
    domX: number;
    domY: number;
    svgX: number;
    record: HistoryRecord;
  };

  const CHART_WIDTH = 720;
  const CHART_HEIGHT = 200;
  const PAD_L = 36;
  const PAD_R = 12;
  const PAD_T = 10;
  const PAD_B = 20;

  // ── State ─────────────────────────────────────────────────────────────────────
  let instances = $state<InstanceInfo[]>([]);
  let selected = $state<string | null>(null);
  let recordsByInstance = $state<Record<string, HistoryRecord[]>>({});
  let loading = $state(false);
  let error = $state<string | null>(null);
  let sinceDays = $state(7);
  let tooltip = $state<TooltipState | null>(null);
  let chartCard = $state<HTMLDivElement | null>(null);

  // ── Derived: base data ────────────────────────────────────────────────────────
  let records = $derived(selected ? (recordsByInstance[selected] ?? []) : []);

  let stats = $derived.by(() => {
    if (!records.length) return null;
    const v5 = records.map((r) => r.five_hour_pct);
    const v5nonzero = v5.filter((v) => v > 0);
    const v7 = records.map((r) => r.seven_day_pct);
    return {
      min5h: v5nonzero.length ? Math.min(...v5nonzero) : 0,
      max5h: Math.max(...v5),
      avg5h: v5.reduce((a, b) => a + b, 0) / v5.length,
      max7d: Math.max(...v7),
      count: records.length,
    };
  });

  // ── Derived: 5-hour windows ───────────────────────────────────────────────────
  let windows = $derived.by((): WindowRow[] => {
    if (!records.length) return [];
    const out: WindowRow[] = [];
    let start = 0;
    let prevResets = records[0].five_hour_resets_at;

    const build = (slice: HistoryRecord[]): WindowRow => {
      const first = slice[0];
      const last = slice[slice.length - 1];
      const peak = Math.max(...slice.map((r) => r.five_hour_pct));
      const hit = slice.some((r) => r.five_hour_pct >= 100);
      const hitRec = hit ? slice.find((r) => r.five_hour_pct >= 100) : undefined;
      const mins = hitRec
        ? Math.round(
            (new Date(hitRec.polled_at).getTime() - new Date(first.polled_at).getTime()) / 60000,
          )
        : null;
      return {
        window_start: first.polled_at,
        window_end: last.polled_at,
        peak_five_hour_pct: peak,
        hit_100: hit,
        time_to_100_mins: mins,
        seven_day_pct_at_start: first.seven_day_pct,
        seven_day_pct_at_end: last.seven_day_pct,
        seven_day_delta: Math.max(0, last.seven_day_pct - first.seven_day_pct),
      };
    };

    for (let i = 1; i < records.length; i++) {
      if (records[i].five_hour_resets_at !== prevResets) {
        out.push(build(records.slice(start, i)));
        start = i;
        prevResets = records[i].five_hour_resets_at;
      }
    }
    out.push(build(records.slice(start)));
    return out.reverse(); // newest first
  });

  // ── Derived: prediction ───────────────────────────────────────────────────────
  let prediction = $derived.by(() => {
    if (!records.length) return null;
    const THEORETICAL = 100 / 33.6;
    const maxed = windows.filter((w) => w.hit_100);
    let avgCost: number;
    let isEmpirical: boolean;
    if (maxed.length >= 2) {
      const raw = maxed.reduce((a, w) => a + w.seven_day_delta, 0) / maxed.length;
      avgCost = raw > 0 ? raw : THEORETICAL;
      isEmpirical = true;
    } else {
      avgCost = THEORETICAL;
      isEmpirical = false;
    }
    const current7d = records[records.length - 1].seven_day_pct;
    return {
      current7d,
      windowsRemaining: Math.max(0, 100 - current7d) / avgCost,
      avgCost,
      isEmpirical,
      windowsObserved: maxed.length,
    };
  });

  // ── Derived: daily pattern (local hours) ──────────────────────────────────────
  let dailyPattern = $derived.by((): (number | null)[] => {
    const sums = new Array(24).fill(0);
    const counts = new Array(24).fill(0);
    for (const r of records) {
      const h = new Date(r.polled_at).getHours();
      sums[h] += r.five_hour_pct;
      counts[h]++;
    }
    return sums.map((s, i) => (counts[i] > 0 ? s / counts[i] : null));
  });

  // ── Derived: chart geometry ───────────────────────────────────────────────────
  let chart = $derived.by(() => {
    if (records.length < 2) return null;

    const times = records.map((r) => new Date(r.polled_at).getTime());
    const tMin = Math.min(...times);
    const tMax = Math.max(...times);
    const tSpan = tMax - tMin || 1;
    const iW = CHART_WIDTH - PAD_L - PAD_R;
    const iH = CHART_HEIGHT - PAD_T - PAD_B;

    const toX = (t: number) => PAD_L + ((t - tMin) / tSpan) * iW;
    const toY = (pct: number) => PAD_T + iH - (Math.min(Math.max(pct, 0), 100) / 100) * iH;

    const pts5h = records.map((r, i) => [toX(times[i]), toY(r.five_hour_pct)] as [number, number]);
    const pts7d = records.map(
      (r, i) => [toX(times[i]), toY(r.seven_day_pct)] as [number, number],
    );

    const pathLine = (pts: [number, number][]) =>
      pts
        .map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`)
        .join(" ");

    const line5h = pathLine(pts5h);
    const line7d = pathLine(pts7d);
    const baseline = PAD_T + iH;
    const area5h = `${line5h} L${pts5h.at(-1)![0].toFixed(1)},${baseline} L${pts5h[0][0].toFixed(1)},${baseline} Z`;

    // Predicted dashed line — only records that have it
    const predPts = records
      .map((r, i) =>
        r.five_hour_predicted_pct != null
          ? ([toX(times[i]), toY(r.five_hour_predicted_pct)] as [number, number])
          : null,
      )
      .filter((p): p is [number, number] => p !== null);
    const predLine = predPts.length >= 2 ? pathLine(predPts) : null;

    // Reset boundary X positions
    const resets: number[] = [];
    for (let i = 1; i < records.length; i++) {
      if (records[i].five_hour_resets_at !== records[i - 1].five_hour_resets_at) {
        resets.push(toX(times[i]));
      }
    }

    // Grid lines (Y)
    const grid = [0, 25, 50, 75, 100].map((p) => ({ y: toY(p), label: `${p}%` }));

    // Time axis ticks
    let tickMs: number;
    let fmtTick: (t: number) => string;
    if (sinceDays <= 1) {
      tickMs = 3 * 3600 * 1000;
      fmtTick = (t) => new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    } else if (sinceDays <= 7) {
      tickMs = 24 * 3600 * 1000;
      fmtTick = (t) => new Date(t).toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" });
    } else {
      tickMs = 2 * 24 * 3600 * 1000;
      fmtTick = (t) => new Date(t).toLocaleDateString([], { month: "short", day: "numeric" });
    }
    const ticks: { x: number; label: string }[] = [];
    let t = Math.ceil(tMin / tickMs) * tickMs;
    while (t <= tMax) {
      ticks.push({ x: toX(t), label: fmtTick(t) });
      t += tickMs;
    }

    return { pts5h, pts7d, line5h, line7d, area5h, predLine, resets, grid, ticks, times, tMin, tSpan, iW, iH, toX, toY };
  });

  // ── Tooltip ───────────────────────────────────────────────────────────────────
  function onMouseMove(e: MouseEvent) {
    if (!chart || !chartCard) return;
    const rect = chartCard.getBoundingClientRect();
    const domX = e.clientX - rect.left;
    const domY = e.clientY - rect.top;
    const svgX = (domX / rect.width) * CHART_WIDTH;
    const t = chart.tMin + ((svgX - PAD_L) / chart.iW) * chart.tSpan;

    let nearestIdx = 0;
    let minDist = Infinity;
    chart.times.forEach((time, i) => {
      const d = Math.abs(time - t);
      if (d < minDist) {
        minDist = d;
        nearestIdx = i;
      }
    });

    tooltip = { domX, domY, svgX: chart.pts5h[nearestIdx][0], record: records[nearestIdx] };
  }

  // ── Data loading ──────────────────────────────────────────────────────────────
  async function loadHistory(instance: string) {
    loading = true;
    error = null;
    try {
      const data = await invoke<HistoryRecord[]>("get_history", { instance, sinceDays });
      recordsByInstance = { ...recordsByInstance, [instance]: data };
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function selectInstance(name: string) {
    selected = name;
    loadHistory(name);
  }

  function setRange(days: number) {
    sinceDays = days;
    recordsByInstance = {};
    if (selected) loadHistory(selected);
  }

  onMount(async () => {
    try {
      instances = await invoke<InstanceInfo[]>("get_instances");
      if (instances.length > 0) selectInstance(instances[0].name);
    } catch (e) {
      console.error("get_instances failed", e);
    }
  });

  // ── Formatters ────────────────────────────────────────────────────────────────
  const fmtPct = (n: number) => `${n.toFixed(0)}%`;
  const fmtTime = (iso: string) =>
    new Date(iso).toLocaleString([], {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  const fmtMins = (m: number) =>
    m < 60 ? `${m}m` : `${Math.floor(m / 60)}h ${m % 60 > 0 ? `${m % 60}m` : ""}`.trim();
  const hourLabel = (h: number) =>
    h === 0 ? "12a" : h === 12 ? "12p" : h < 12 ? `${h}a` : `${h - 12}p`;
</script>

<div class="page">
  <div class="page-header">
    <div>
      <h1>History</h1>
      <p class="subtitle">
        Usage trends · last {sinceDays === 1 ? "24 hours" : `${sinceDays} days`}
      </p>
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
              onclick={() => selectInstance(inst.name)}
            >
              {inst.name}
            </button>
          {/each}
        </div>
      {/if}
      <div class="range-selector" role="group" aria-label="Time range">
        {#each [1, 7, 14] as days}
          <button
            class="range-btn"
            class:active={sinceDays === days}
            onclick={() => setRange(days)}
          >
            {days === 1 ? "24h" : `${days}d`}
          </button>
        {/each}
      </div>
    </div>
  </div>

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
      <p class="sub">Polls will appear here once the monitor has run a few cycles.</p>
    </div>
  {:else}
    <!-- Prediction card -->
    {#if prediction}
      <div
        class="prediction-card"
        class:warn={prediction.windowsRemaining >= 3 && prediction.windowsRemaining < 8}
        class:danger={prediction.windowsRemaining < 3}
      >
        <div class="pred-main">
          <span class="pred-number">{prediction.windowsRemaining.toFixed(1)}</span>
          <span class="pred-label">5-hour windows remaining before 7-day limit</span>
        </div>
        <div class="pred-meta">
          <span>7-day: {fmtPct(prediction.current7d)}</span>
          <span>·</span>
          <span>~{prediction.avgCost.toFixed(2)}% per window</span>
          <span>·</span>
          <span class="pred-source" class:empirical={prediction.isEmpirical}>
            {prediction.isEmpirical
              ? `empirical (${prediction.windowsObserved} windows)`
              : "theoretical estimate"}
          </span>
        </div>
      </div>
    {/if}

    <!-- Chart -->
    <div
      class="chart-card"
      bind:this={chartCard}
      role="img"
      aria-label="Usage chart"
      onmousemove={onMouseMove}
      onmouseleave={() => (tooltip = null)}
    >
      <!-- Tooltip bubble -->
      {#if tooltip && chartCard}
        <div
          class="tooltip"
          style:left={tooltip.domX < chartCard.clientWidth / 2
            ? `${tooltip.domX + 14}px`
            : "auto"}
          style:right={tooltip.domX >= chartCard.clientWidth / 2
            ? `${chartCard.clientWidth - tooltip.domX + 14}px`
            : "auto"}
          style:top={`${Math.max(4, tooltip.domY - 90)}px`}
        >
          <div class="tt-time">{fmtTime(tooltip.record.polled_at)}</div>
          <div class="tt-row">
            <span class="tt-dot dot-5h"></span>
            <span>5-hour</span>
            <strong>{fmtPct(tooltip.record.five_hour_pct)}</strong>
          </div>
          <div class="tt-row">
            <span class="tt-dot dot-7d"></span>
            <span>7-day</span>
            <strong>{fmtPct(tooltip.record.seven_day_pct)}</strong>
          </div>
          {#if tooltip.record.five_hour_predicted_pct != null}
            <div class="tt-row">
              <span class="tt-dot dot-pred"></span>
              <span>predicted</span>
              <strong>{fmtPct(tooltip.record.five_hour_predicted_pct)}</strong>
            </div>
          {/if}
        </div>
      {/if}

      <svg viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}>
        {#if chart}
          <!-- Grid lines -->
          {#each chart.grid as g}
            <line
              x1={PAD_L}
              x2={CHART_WIDTH - PAD_R}
              y1={g.y}
              y2={g.y}
              class="grid-line"
            />
            <text x={PAD_L - 4} y={g.y + 4} class="grid-label" text-anchor="end"
              >{g.label}</text
            >
          {/each}

          <!-- Reset boundaries -->
          {#each chart.resets as rx}
            <line
              x1={rx}
              x2={rx}
              y1={PAD_T}
              y2={PAD_T + chart.iH}
              class="reset-line"
            />
          {/each}

          <!-- 5h area fill -->
          <path d={chart.area5h} class="area-fill-5h" />

          <!-- 7-day line -->
          <path d={chart.line7d} class="line-7d" />

          <!-- 5h line -->
          <path d={chart.line5h} class="line-5h" />

          <!-- Predicted dashed line -->
          {#if chart.predLine}
            <path d={chart.predLine} class="line-pred" />
          {/if}

          <!-- Crosshair + dots -->
          {#if tooltip}
            <line
              x1={tooltip.svgX}
              x2={tooltip.svgX}
              y1={PAD_T}
              y2={PAD_T + chart.iH}
              class="crosshair"
            />
            <circle
              cx={tooltip.svgX}
              cy={chart.toY(tooltip.record.five_hour_pct)}
              r="3.5"
              class="dot-circle-5h"
            />
            <circle
              cx={tooltip.svgX}
              cy={chart.toY(tooltip.record.seven_day_pct)}
              r="3"
              class="dot-circle-7d"
            />
          {/if}

          <!-- Time axis -->
          {#each chart.ticks as tick}
            <text
              x={tick.x}
              y={CHART_HEIGHT - 3}
              class="time-label"
              text-anchor="middle">{tick.label}</text
            >
          {/each}
        {/if}
      </svg>

      <!-- Legend -->
      <div class="chart-legend">
        <span class="legend-item">
          <span class="legend-dot" style="background: hsl(222 84% 50%)"></span>5-hour
        </span>
        <span class="legend-item">
          <span class="legend-dot" style="background: hsl(25 95% 55%)"></span>7-day
        </span>
        {#if chart?.predLine}
          <span class="legend-item">
            <span class="legend-dash"></span>predicted
          </span>
        {/if}
        {#if chart && chart.resets.length > 0}
          <span class="legend-item">
            <span class="legend-reset"></span>window reset
          </span>
        {/if}
      </div>
    </div>

    <!-- Stats row -->
    {#if stats}
      <div class="stats-row">
        <div class="stat">
          <span class="stat-label">Low 5h</span>
          <span class="stat-value">{fmtPct(stats.min5h)}</span>
        </div>
        <div class="stat">
          <span class="stat-label">Max 5h</span>
          <span class="stat-value">{fmtPct(stats.max5h)}</span>
        </div>
        <div class="stat">
          <span class="stat-label">Avg 5h</span>
          <span class="stat-value">{fmtPct(stats.avg5h)}</span>
        </div>
        <div class="stat">
          <span class="stat-label">Max 7d</span>
          <span class="stat-value">{fmtPct(stats.max7d)}</span>
        </div>
        <div class="stat">
          <span class="stat-label">Polls</span>
          <span class="stat-value">{stats.count}</span>
        </div>
      </div>
    {/if}

    <!-- Windows breakdown -->
    {#if windows.length > 0}
      <section class="section">
        <h2>5-Hour Windows</h2>
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Window start</th>
                <th>Peak 5h</th>
                <th>Hit limit</th>
                <th>Time to limit</th>
                <th>7d cost</th>
              </tr>
            </thead>
            <tbody>
              {#each windows as w (w.window_start)}
                <tr>
                  <td>{fmtTime(w.window_start)}</td>
                  <td>
                    <span
                      class="pct-text"
                      class:pct-high={w.peak_five_hour_pct >= 80}
                      class:pct-mid={w.peak_five_hour_pct >= 50 && w.peak_five_hour_pct < 80}
                    >
                      {fmtPct(w.peak_five_hour_pct)}
                    </span>
                  </td>
                  <td>
                    {#if w.hit_100}
                      <span class="badge badge-hit">Yes</span>
                    {:else}
                      <span class="badge badge-no">No</span>
                    {/if}
                  </td>
                  <td>{w.time_to_100_mins != null ? fmtMins(w.time_to_100_mins) : "—"}</td>
                  <td class="delta">
                    {w.seven_day_delta > 0 ? `+${w.seven_day_delta.toFixed(1)}%` : "—"}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    {/if}

    <!-- Daily pattern -->
    {#if records.length >= 24}
      <section class="section">
        <h2>Daily Pattern</h2>
        <p class="section-sub">Average 5-hour usage by hour of day (local time)</p>
        <div class="daily-chart">
          {#each dailyPattern as val, h}
            <div class="hour-col">
              <div class="bar-track">
                {#if val != null}
                  <div
                    class="bar-fill"
                    class:bar-high={val >= 80}
                    class:bar-mid={val >= 50 && val < 80}
                    style:height="{val}%"
                  ></div>
                {/if}
              </div>
              <div class="hour-label">{h % 3 === 0 ? hourLabel(h) : ""}</div>
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .page {
    padding: 2rem;
    max-width: 860px;
  }

  /* ── Header ─────────────────────────────────────────────────────────────────── */
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
    margin-bottom: 1.25rem;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.2rem;
    color: hsl(222.2 84% 4.9%);
  }

  .subtitle {
    color: hsl(215.4 16.3% 46.9%);
    font-size: 0.875rem;
    margin: 0;
  }

  .controls {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.5rem;
  }

  /* ── Instance tabs ──────────────────────────────────────────────────────────── */
  .tabs {
    display: flex;
    gap: 0.25rem;
    border-bottom: 1px solid hsl(214.3 31.8% 91.4%);
  }

  .tab {
    padding: 0.4rem 0.85rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 0.8125rem;
    color: hsl(215.4 16.3% 46.9%);
    cursor: pointer;
  }

  .tab.active {
    color: hsl(222.2 84% 4.9%);
    border-bottom-color: hsl(222.2 84% 4.9%);
    font-weight: 600;
  }

  /* ── Range selector ─────────────────────────────────────────────────────────── */
  .range-selector {
    display: flex;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: 6px;
    overflow: hidden;
  }

  .range-btn {
    padding: 0.3rem 0.7rem;
    background: none;
    border: none;
    border-right: 1px solid hsl(214.3 31.8% 91.4%);
    font-size: 0.8125rem;
    color: hsl(215.4 16.3% 46.9%);
    cursor: pointer;
  }

  .range-btn:last-child {
    border-right: none;
  }

  .range-btn.active {
    background: hsl(222.2 84% 4.9%);
    color: hsl(0 0% 100%);
  }

  /* ── Error / loading / empty ────────────────────────────────────────────────── */
  .error-banner {
    padding: 0.75rem 1rem;
    background: hsl(0 84% 95%);
    color: hsl(0 70% 40%);
    border-radius: var(--radius);
    font-size: 0.875rem;
    margin-bottom: 1rem;
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
  }

  /* ── Prediction card ────────────────────────────────────────────────────────── */
  .prediction-card {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 1rem 1.25rem;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    background: hsl(0 0% 100%);
    margin-bottom: 1rem;
  }

  .prediction-card.warn {
    border-color: hsl(38 92% 60%);
    background: hsl(48 100% 97%);
  }

  .prediction-card.danger {
    border-color: hsl(0 84% 65%);
    background: hsl(0 86% 97%);
  }

  .pred-main {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
  }

  .pred-number {
    font-size: 2rem;
    font-weight: 700;
    color: hsl(222.2 84% 4.9%);
    line-height: 1;
  }

  .prediction-card.warn .pred-number {
    color: hsl(38 92% 32%);
  }

  .prediction-card.danger .pred-number {
    color: hsl(0 70% 40%);
  }

  .pred-label {
    font-size: 0.875rem;
    color: hsl(215.4 16.3% 46.9%);
  }

  .pred-meta {
    display: flex;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: hsl(215.4 16.3% 56%);
    flex-wrap: wrap;
  }

  .pred-source.empirical {
    color: hsl(142 70% 35%);
  }

  /* ── Chart card ─────────────────────────────────────────────────────────────── */
  .chart-card {
    position: relative;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    padding: 0.75rem 0.75rem 0.4rem;
    background: hsl(0 0% 100%);
    margin-bottom: 1rem;
    cursor: crosshair;
    user-select: none;
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
    font-family: inherit;
  }

  .time-label {
    font-size: 8px;
    fill: hsl(215.4 16.3% 60%);
    font-family: inherit;
  }

  .reset-line {
    stroke: hsl(215 25% 72%);
    stroke-width: 1;
    stroke-dasharray: 4 3;
  }

  .area-fill-5h {
    fill: hsl(222 84% 55% / 0.1);
  }

  .line-5h {
    fill: none;
    stroke: hsl(222 84% 50%);
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
  }

  .line-7d {
    fill: none;
    stroke: hsl(25 95% 55%);
    stroke-width: 1.5;
    stroke-linejoin: round;
    stroke-linecap: round;
  }

  .line-pred {
    fill: none;
    stroke: hsl(222 84% 55%);
    stroke-width: 1.5;
    stroke-dasharray: 5 3;
    opacity: 0.5;
  }

  .crosshair {
    stroke: hsl(215 15% 65%);
    stroke-width: 1;
    stroke-dasharray: 3 2;
    pointer-events: none;
  }

  .dot-circle-5h {
    fill: hsl(222 84% 50%);
    pointer-events: none;
  }

  .dot-circle-7d {
    fill: hsl(25 95% 55%);
    pointer-events: none;
  }

  /* Legend */
  .chart-legend {
    display: flex;
    gap: 1rem;
    padding: 0.35rem 0.25rem 0;
    font-size: 0.75rem;
    color: hsl(215.4 16.3% 46.9%);
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .legend-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .legend-dash {
    width: 18px;
    height: 0;
    border-top: 2px dashed hsl(222 84% 55%);
    opacity: 0.55;
    flex-shrink: 0;
  }

  .legend-reset {
    width: 18px;
    height: 0;
    border-top: 1px dashed hsl(215 25% 72%);
    flex-shrink: 0;
  }

  /* ── Tooltip ────────────────────────────────────────────────────────────────── */
  .tooltip {
    position: absolute;
    pointer-events: none;
    z-index: 10;
    background: hsl(222.2 84% 6%);
    color: hsl(0 0% 96%);
    border-radius: 6px;
    padding: 0.5rem 0.7rem;
    font-size: 0.75rem;
    min-width: 158px;
    box-shadow: 0 4px 14px hsl(0 0% 0% / 0.3);
  }

  .tt-time {
    font-size: 0.7rem;
    opacity: 0.6;
    margin-bottom: 0.4rem;
  }

  .tt-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.18rem;
    color: hsl(0 0% 78%);
  }

  .tt-row:last-child {
    margin-bottom: 0;
  }

  .tt-row strong {
    margin-left: auto;
    color: hsl(0 0% 97%);
    font-weight: 600;
  }

  .tt-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot-5h {
    background: hsl(222 84% 65%);
  }

  .dot-7d {
    background: hsl(25 95% 62%);
  }

  .dot-pred {
    background: hsl(222 84% 65%);
    opacity: 0.55;
  }

  /* ── Stats row ──────────────────────────────────────────────────────────────── */
  .stats-row {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 0.75rem;
    margin-bottom: 1.5rem;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.7rem 0.85rem;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    background: hsl(0 0% 100%);
  }

  .stat-label {
    font-size: 0.7rem;
    color: hsl(215.4 16.3% 46.9%);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .stat-value {
    font-size: 1.1rem;
    font-weight: 600;
    color: hsl(222.2 84% 4.9%);
  }

  /* ── Section ────────────────────────────────────────────────────────────────── */
  .section {
    margin-bottom: 2rem;
  }

  h2 {
    font-size: 0.9375rem;
    font-weight: 600;
    color: hsl(222.2 84% 4.9%);
    margin: 0 0 0.2rem;
  }

  .section-sub {
    font-size: 0.8rem;
    color: hsl(215.4 16.3% 46.9%);
    margin: 0 0 0.75rem;
  }

  /* ── Windows table ──────────────────────────────────────────────────────────── */
  .table-wrap {
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    overflow: hidden;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8125rem;
  }

  thead tr {
    background: hsl(210 40% 98%);
  }

  th {
    padding: 0.55rem 0.85rem;
    text-align: left;
    font-weight: 600;
    font-size: 0.7rem;
    color: hsl(215.4 16.3% 46.9%);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid hsl(214.3 31.8% 91.4%);
  }

  td {
    padding: 0.5rem 0.85rem;
    border-bottom: 1px solid hsl(214.3 31.8% 91.4%);
    color: hsl(222.2 84% 4.9%);
  }

  tbody tr:last-child td {
    border-bottom: none;
  }

  tbody tr:hover td {
    background: hsl(210 40% 98.5%);
  }

  .pct-text {
    font-weight: 600;
  }

  .pct-high {
    color: hsl(0 70% 45%);
  }

  .pct-mid {
    color: hsl(38 92% 32%);
  }

  .badge {
    display: inline-block;
    padding: 0.1rem 0.42rem;
    border-radius: 4px;
    font-size: 0.72rem;
    font-weight: 600;
  }

  .badge-hit {
    background: hsl(0 84% 93%);
    color: hsl(0 70% 38%);
  }

  .badge-no {
    background: hsl(142 60% 92%);
    color: hsl(142 55% 28%);
  }

  .delta {
    color: hsl(215.4 16.3% 46.9%);
    font-variant-numeric: tabular-nums;
  }

  /* ── Daily pattern ──────────────────────────────────────────────────────────── */
  .daily-chart {
    display: flex;
    gap: 2px;
    height: 96px;
  }

  .hour-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .bar-track {
    flex: 1;
    background: hsl(214.3 31.8% 93%);
    border-radius: 2px 2px 0 0;
    display: flex;
    align-items: flex-end;
    overflow: hidden;
  }

  .bar-fill {
    width: 100%;
    background: hsl(222 84% 58%);
    border-radius: 2px 2px 0 0;
    min-height: 1px;
  }

  .bar-high {
    background: hsl(0 70% 55%);
  }

  .bar-mid {
    background: hsl(38 92% 50%);
  }

  .hour-label {
    font-size: 0.6rem;
    color: hsl(215.4 16.3% 56%);
    text-align: center;
    height: 14px;
    line-height: 14px;
    white-space: nowrap;
  }
</style>
