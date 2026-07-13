<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { theme } from "../lib/theme.svelte";

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
    peak_five_hour_pct: number;
    seven_day_delta: number;
  };

  type ResetEvent = {
    resetAt: string; // window boundary — the scheduled reset time
    detectedAt: string; // poll when the fresh window was first seen
    windowType: "5h" | "7d";
    peakPct: number; // peak usage in the window that just ended
    hitCap: boolean; // window reached 100%
    lagMinutes: number; // detection lag (detectedAt − resetAt)
  };

  type TooltipState = {
    domX: number;
    domY: number;
    svgX: number;
    record: HistoryRecord;
  };

  type DisplayRange = "5h" | "24h" | "7d" | "14d" | "21d";

  // A window's reset timestamp is reported as `now + remaining`, so it jitters
  // by fractions of a second on every poll. A *genuine* reset is when it jumps
  // forward by far more than that jitter — real windows are hours apart.
  const RESET_JUMP_MS = 30 * 60 * 1000;

  function jumpedForward(prev: string | null, curr: string | null): boolean {
    if (!prev || !curr) return false;
    return new Date(curr).getTime() - new Date(prev).getTime() > RESET_JUMP_MS;
  }

  // Boundary for grouping records into windows: a real forward jump, or a
  // transition into/out of "no active window" (null).
  function isWindowBoundary(prev: string | null, curr: string | null): boolean {
    if (prev === curr) return false;
    if (!prev || !curr) return true;
    return jumpedForward(prev, curr);
  }

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
  let displayRange = $state<DisplayRange>("7d");
  let tooltip = $state<TooltipState | null>(null);
  let chartCard = $state<HTMLDivElement | null>(null);
  let viewMode = $state<"overview" | "resets">("overview");
  let heatTooltip = $state<{ domX: number; domY: number; label: string; val: number } | null>(
    null
  );
  let heatmapCard = $state<HTMLDivElement | null>(null);

  function rangeToDays(r: DisplayRange): number {
    if (r === "5h" || r === "24h") return 1;
    if (r === "7d") return 7;
    if (r === "14d") return 14;
    return 21;
  }

  let sinceDays = $derived(rangeToDays(displayRange));

  // ── Derived: base data ────────────────────────────────────────────────────────
  let allRecords = $derived(selected ? (recordsByInstance[selected] ?? []) : []);

  // For 5h view: show only current 5h window
  let records = $derived.by(() => {
    if (displayRange !== "5h" || allRecords.length === 0) return allRecords;
    const last = allRecords[allRecords.length - 1];
    const currentResetAt = last.five_hour_resets_at;
    if (!currentResetAt) return allRecords.slice(-20);
    // Same window = reset timestamp within jitter range of the current one.
    const ref = new Date(currentResetAt).getTime();
    return allRecords.filter(
      (r) =>
        r.five_hour_resets_at != null &&
        Math.abs(new Date(r.five_hour_resets_at).getTime() - ref) < RESET_JUMP_MS
    );
  });

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
    const build = (slice: HistoryRecord[]): WindowRow => ({
      peak_five_hour_pct: Math.max(...slice.map((r) => r.five_hour_pct)),
      seven_day_delta: Math.max(0, slice[slice.length - 1].seven_day_pct - slice[0].seven_day_pct),
    });
    for (let i = 1; i < records.length; i++) {
      if (isWindowBoundary(prevResets, records[i].five_hour_resets_at)) {
        out.push(build(records.slice(start, i)));
        start = i;
      }
      prevResets = records[i].five_hour_resets_at;
    }
    out.push(build(records.slice(start)));
    return out;
  });

  // ── Derived: prediction ───────────────────────────────────────────────────────
  let prediction = $derived.by(() => {
    if (!records.length) return null;
    const THEORETICAL = 100 / 33.6;
    let consumed = 0;
    let fullEquiv = 0;
    for (const w of windows) {
      if (w.seven_day_delta > 0 && w.peak_five_hour_pct > 0) {
        consumed += w.seven_day_delta;
        fullEquiv += w.peak_five_hour_pct / 100;
      }
    }
    const empCost = fullEquiv > 0 ? consumed / fullEquiv : 0;
    let avgCost: number;
    let isEmpirical: boolean;
    if (fullEquiv >= 3 && empCost >= 0.3 && empCost <= 15) {
      avgCost = empCost;
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
      windowsObserved: Math.round(fullEquiv),
    };
  });

  // ── Derived: current 5h window burndown ──────────────────────────────────────
  let currentWindow = $derived.by(() => {
    if (records.length < 2) return null;
    const last = records[records.length - 1];
    const first = records[0];
    const resetIso = last.five_hour_resets_at;
    if (!resetIso) return null;
    const WINDOW_MS = 5 * 3600 * 1000;
    const resetAt = new Date(resetIso).getTime();
    const windowStart = resetAt - WINDOW_MS;
    const now = new Date(last.polled_at).getTime();
    const elapsed = Math.max(0, now - windowStart);
    const remaining = Math.max(0, resetAt - now);
    const current = last.five_hour_pct;
    const spanMs = now - new Date(first.polled_at).getTime();
    const burnRate = spanMs > 0 ? (current - first.five_hour_pct) / spanMs : 0;
    return {
      current,
      resetsAt: resetAt,
      remainingMs: remaining,
      elapsedFraction: Math.min(1, elapsed / WINDOW_MS),
      projectedPeak: Math.min(100, burnRate > 0 ? current + burnRate * remaining : current),
      burnRatePerHour: burnRate * 3600000,
    };
  });

  // ── Derived: 7-day pace ───────────────────────────────────────────────────────
  let pace = $derived.by(() => {
    if (records.length < 2) return null;
    const last = records[records.length - 1];
    const current = last.seven_day_pct;
    const resetIso = last.seven_day_resets_at;
    if (!resetIso) return null;
    const DAY = 86_400_000;
    const WINDOW = 7 * DAY;
    const resetAt = new Date(resetIso).getTime();
    const now = new Date(last.polled_at).getTime();
    const windowStart = resetAt - WINDOW;
    const elapsed = Math.min(Math.max(now - windowStart, 0), WINDOW);
    const evenPacePct = (elapsed / WINDOW) * 100;
    const paceDiff = current - evenPacePct;
    const daysToReset = Math.max(0, (resetAt - now) / DAY);
    const safePerDay = daysToReset > 0.01 ? (100 - current) / daysToReset : 0;
    const inWindow = records.filter((r) => r.seven_day_resets_at === resetIso);
    let burnPerDay: number | null = null;
    if (inWindow.length >= 2) {
      const f = inWindow[0];
      const spanDays = (now - new Date(f.polled_at).getTime()) / DAY;
      if (spanDays > 0.04) burnPerDay = (current - f.seven_day_pct) / spanDays;
    }
    let verdict: "spare" | "track" | "ease";
    if (paceDiff > 10) verdict = "ease";
    else if (paceDiff < -10) verdict = "spare";
    else verdict = "track";
    return { current, evenPacePct, paceDiff, daysToReset, safePerDay, burnPerDay, verdict, resetAt };
  });

  // ── Derived: daily pattern ────────────────────────────────────────────────────
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

  // ── Derived: day×hour heatmap ─────────────────────────────────────────────────
  let heatmap = $derived.by((): (number | null)[][] => {
    // rows = 0..6 (Sun-Sat), cols = 0..23
    const sums = Array.from({ length: 7 }, () => new Array(24).fill(0));
    const counts = Array.from({ length: 7 }, () => new Array(24).fill(0));
    for (const r of records) {
      const d = new Date(r.polled_at);
      sums[d.getDay()][d.getHours()] += r.five_hour_pct;
      counts[d.getDay()][d.getHours()]++;
    }
    return sums.map((row, dow) =>
      row.map((s, h) => (counts[dow][h] > 0 ? s / counts[dow][h] : null))
    );
  });

  // ── Derived: week-over-week ───────────────────────────────────────────────────
  let weekComparison = $derived.by(() => {
    if (allRecords.length < 2) return [];
    const weeks = new Map<string, HistoryRecord[]>();
    for (const r of allRecords) {
      const d = new Date(r.polled_at);
      const day = d.getDay();
      const diff = d.getDate() - day + (day === 0 ? -6 : 1);
      const mon = new Date(d);
      mon.setDate(diff);
      mon.setHours(0, 0, 0, 0);
      const key = mon.toISOString().slice(0, 10);
      if (!weeks.has(key)) weeks.set(key, []);
      weeks.get(key)!.push(r);
    }
    const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    return Array.from(weeks.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .slice(-4)
      .map(([weekStart, recs]) => {
        const dayMap = new Map<number, HistoryRecord[]>();
        for (const r of recs) {
          const dow = new Date(r.polled_at).getDay();
          const idx = dow === 0 ? 6 : dow - 1; // Mon=0..Sun=6
          if (!dayMap.has(idx)) dayMap.set(idx, []);
          dayMap.get(idx)!.push(r);
        }
        const days = DAY_LABELS.map((label, i) => {
          const dayRecs = dayMap.get(i) ?? [];
          const consumption =
            dayRecs.length >= 2
              ? Math.max(0, dayRecs[dayRecs.length - 1].seven_day_pct - dayRecs[0].seven_day_pct)
              : 0;
          return { label, consumption };
        });
        const mon = new Date(weekStart);
        const sun = new Date(mon);
        sun.setDate(mon.getDate() + 6);
        return {
          weekStart,
          label: `${mon.toLocaleDateString([], { month: "short", day: "numeric" })} – ${sun.toLocaleDateString([], { month: "short", day: "numeric" })}`,
          days,
          total: days.reduce((s, d) => s + d.consumption, 0),
        };
      });
  });

  // ── Derived: reset events ─────────────────────────────────────────────────────
  // One event per genuine window boundary. For each, the peak usage recorded in
  // the window that just ended — i.e. how hard that window was hit before it reset.
  let resets = $derived.by((): ResetEvent[] => {
    if (allRecords.length < 2) return [];
    const events: ResetEvent[] = [];

    const scan = (
      type: "5h" | "7d",
      resetKey: "five_hour_resets_at" | "seven_day_resets_at",
      pctKey: "five_hour_pct" | "seven_day_pct"
    ) => {
      let start = 0;
      // Track the last non-null reset timestamp so a window that resets while
      // usage is idle (reset_at momentarily null) is still detected.
      let lastReset = allRecords[0][resetKey];
      for (let i = 1; i < allRecords.length; i++) {
        const curr = allRecords[i][resetKey];
        if (jumpedForward(lastReset, curr)) {
          let peak = 0;
          for (let k = start; k < i; k++) peak = Math.max(peak, allRecords[k][pctKey]);
          events.push({
            resetAt: lastReset!,
            detectedAt: allRecords[i].polled_at,
            windowType: type,
            peakPct: peak,
            hitCap: peak >= 100,
            lagMinutes:
              (new Date(allRecords[i].polled_at).getTime() - new Date(lastReset!).getTime()) /
              60000,
          });
          start = i;
        }
        if (curr) lastReset = curr;
      }
    };

    scan("5h", "five_hour_resets_at", "five_hour_pct");
    scan("7d", "seven_day_resets_at", "seven_day_pct");

    return events.sort((a, b) => new Date(b.resetAt).getTime() - new Date(a.resetAt).getTime());
  });

  let resetSummary = $derived.by(() => {
    const r5 = resets.filter((r) => r.windowType === "5h");
    return {
      count5h: r5.length,
      count7d: resets.length - r5.length,
      maxed: r5.filter((r) => r.hitCap).length,
      avgPeak: r5.length ? r5.reduce((a, b) => a + b.peakPct, 0) / r5.length : 0,
    };
  });

  // Group resets by calendar day (newest first) for the timeline.
  let resetsByDay = $derived.by((): { key: string; label: string; events: ResetEvent[] }[] => {
    const map = new Map<string, ResetEvent[]>();
    for (const e of resets) {
      const key = new Date(e.resetAt).toDateString();
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(e);
    }
    const today = new Date().toDateString();
    const yest = new Date(Date.now() - 86_400_000).toDateString();
    return Array.from(map.entries()).map(([key, events]) => ({
      key,
      label:
        key === today
          ? "Today"
          : key === yest
            ? "Yesterday"
            : new Date(key).toLocaleDateString([], {
                weekday: "long",
                month: "short",
                day: "numeric",
              }),
      events,
    }));
  });

  // ── Derived: main chart ───────────────────────────────────────────────────────
  let chart = $derived.by(() => {
    if (records.length < 2) return null;
    const times = records.map((r) => new Date(r.polled_at).getTime());
    const tMin = Math.min(...times);
    const tMax = Math.max(...times);
    const tSpan = tMax - tMin || 1;
    const iW = CHART_WIDTH - PAD_L - PAD_R;
    const iH = CHART_HEIGHT - PAD_T - PAD_B;
    const toX = (t: number) => PAD_L + ((t - tMin) / tSpan) * iW;
    const toY = (pct: number) =>
      PAD_T + iH - (Math.min(Math.max(pct, 0), 100) / 100) * iH;
    const pts5h = records.map((r, i) => [toX(times[i]), toY(r.five_hour_pct)] as [number, number]);
    const pts7d = records.map(
      (r, i) => [toX(times[i]), toY(r.seven_day_pct)] as [number, number]
    );
    const pathLine = (pts: [number, number][]) =>
      pts.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
    const line5h = pathLine(pts5h);
    const line7d = pathLine(pts7d);
    const baseline = PAD_T + iH;
    const area5h = `${line5h} L${pts5h.at(-1)![0].toFixed(1)},${baseline} L${pts5h[0][0].toFixed(1)},${baseline} Z`;
    const predPts = records
      .map((r, i) =>
        r.five_hour_predicted_pct != null
          ? ([toX(times[i]), toY(r.five_hour_predicted_pct)] as [number, number])
          : null
      )
      .filter((p): p is [number, number] => p !== null);
    const predLine = predPts.length >= 2 ? pathLine(predPts) : null;
    const resetLines: number[] = [];
    for (let i = 1; i < records.length; i++) {
      if (isWindowBoundary(records[i - 1].five_hour_resets_at, records[i].five_hour_resets_at))
        resetLines.push(toX(times[i]));
    }
    const grid = [0, 25, 50, 75, 100].map((p) => ({ y: toY(p), label: `${p}%` }));
    const effectiveDays = displayRange === "5h" ? 0.21 : sinceDays;
    let tickMs: number;
    let fmtTick: (t: number) => string;
    if (effectiveDays <= 1) {
      tickMs = 3 * 3600 * 1000;
      fmtTick = (t) => new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    } else if (effectiveDays <= 7) {
      tickMs = 24 * 3600 * 1000;
      fmtTick = (t) =>
        new Date(t).toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" });
    } else {
      tickMs = 2 * 24 * 3600 * 1000;
      fmtTick = (t) => new Date(t).toLocaleDateString([], { month: "short", day: "numeric" });
    }
    const ticks: { x: number; label: string }[] = [];
    let tk = Math.ceil(tMin / tickMs) * tickMs;
    while (tk <= tMax) {
      ticks.push({ x: toX(tk), label: fmtTick(tk) });
      tk += tickMs;
    }
    return {
      pts5h, pts7d, line5h, line7d, area5h, predLine,
      resets: resetLines, grid, ticks, times, tMin, tSpan, iW, iH, toX, toY,
    };
  });

  // ── Tooltip: main chart ───────────────────────────────────────────────────────
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
      if (d < minDist) { minDist = d; nearestIdx = i; }
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

  function setRange(range: DisplayRange) {
    const newDays = rangeToDays(range);
    const oldDays = rangeToDays(displayRange);
    displayRange = range;
    if (newDays !== oldDays) {
      recordsByInstance = {};
      if (selected) loadHistory(selected);
    }
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
    new Date(iso).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  const hourLabel = (h: number) =>
    h === 0 ? "12a" : h === 12 ? "12p" : h < 12 ? `${h}a` : `${h - 12}p`;
  const fmtRate = (n: number) => `${n.toFixed(1)}%/day`;
  const fmtClock = (iso: string) =>
    new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

  function onHeatMove(e: MouseEvent, dow: number, hour: number, val: number | null) {
    if (val == null || !heatmapCard) {
      heatTooltip = null;
      return;
    }
    const rect = heatmapCard.getBoundingClientRect();
    heatTooltip = {
      domX: e.clientX - rect.left,
      domY: e.clientY - rect.top,
      label: `${DOW_LABELS[dow]} · ${hourLabel(hour)}`,
      val,
    };
  }
  const fmtDays = (d: number) => d < 1 ? `${Math.round(d * 24)}h` : `${d.toFixed(d < 2 ? 1 : 0)}d`;
  const fmtDuration = (ms: number) => {
    const h = Math.floor(ms / 3600000);
    const m = Math.floor((ms % 3600000) / 60000);
    return h > 0 ? `${h}h ${m}m` : `${m}m`;
  };
  const fmtResetDate = (ms: number) =>
    new Date(ms).toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" });

  const PACE_COPY = {
    spare: {
      title: "Room to spare — push harder",
      tone: "You're behind your even-pace line. You can lean in without risking the weekly limit.",
    },
    track: {
      title: "Right on pace",
      tone: "You're tracking to spend the weekly budget evenly through the reset. Keep going.",
    },
    ease: {
      title: "Ahead of pace — ease off",
      tone: "At this rate you'll exhaust the weekly limit before it resets. Consider slowing down.",
    },
  } as const;

  const DOW_LABELS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

  let maxWeekDay = $derived(
    Math.max(1, ...weekComparison.flatMap((w) => w.days.map((d) => d.consumption)))
  );

  let maxHeatmapCell = $derived(
    Math.max(1, ...heatmap.flat().filter((v): v is number => v !== null))
  );
</script>

<div class="page">
  <!-- ── Header ─────────────────────────────────────────────────────────────── -->
  <div class="page-header">
    <div>
      <h1>History</h1>
      <p class="subtitle">
        {displayRange === "5h"
          ? "Current 5-hour window"
          : displayRange === "24h"
            ? "Last 24 hours"
            : displayRange === "7d"
              ? "Last 7 days"
              : displayRange === "14d"
                ? "Last 14 days"
                : "Last 3 weeks"}
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
              onclick={() => selectInstance(inst.name)}>{inst.name}</button
            >
          {/each}
        </div>
      {/if}
      <div class="view-tabs" role="tablist" aria-label="View">
        <button
          class="view-tab"
          class:active={viewMode === "overview"}
          role="tab"
          aria-selected={viewMode === "overview"}
          onclick={() => (viewMode = "overview")}>Overview</button
        >
        <button
          class="view-tab"
          class:active={viewMode === "resets"}
          role="tab"
          aria-selected={viewMode === "resets"}
          onclick={() => (viewMode = "resets")}>Resets</button
        >
      </div>
      <div class="range-selector" role="group" aria-label="Time range">
        {#each (["5h", "24h", "7d", "14d", "21d"] as DisplayRange[]) as range}
          <button
            class="range-btn"
            class:active={displayRange === range}
            onclick={() => setRange(range)}>{range}</button
          >
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
  {:else if records.length === 0 && allRecords.length === 0}
    <div class="empty-state">
      <p>No history recorded yet for this instance.</p>
      <p class="sub">Polls will appear here once the monitor has run a few cycles.</p>
    </div>
  {:else}
    {#if viewMode === "overview"}
      <!-- Prediction card (not shown in 5h view) -->
      {#if prediction && displayRange !== "5h"}
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

      <!-- 5h burndown card -->
      {#if displayRange === "5h" && currentWindow}
        {@const proj = currentWindow.projectedPeak}
        <div
          class="window-card"
          class:window-warn={proj >= 75 && proj < 95}
          class:window-danger={proj >= 95}
        >
          <div class="window-stats">
            <div class="window-stat">
              <span class="window-stat-label">Now</span>
              <span class="window-stat-value">{fmtPct(currentWindow.current)}</span>
            </div>
            <div class="window-stat">
              <span class="window-stat-label">Projected peak</span>
              <span class="window-stat-value">{fmtPct(proj)}</span>
            </div>
            <div class="window-stat">
              <span class="window-stat-label">Resets in</span>
              <span class="window-stat-value">{fmtDuration(currentWindow.remainingMs)}</span>
            </div>
            <div class="window-stat">
              <span class="window-stat-label">Burn rate</span>
              <span class="window-stat-value">{currentWindow.burnRatePerHour.toFixed(1)}%/h</span>
            </div>
          </div>
          <!-- Dual-layer bar: projected (ghost) + current -->
          <div class="window-bar-wrap">
            <div class="window-bar">
              {#if proj > currentWindow.current}
                <div class="window-bar-proj" style:width="{proj}%"></div>
              {/if}
              <div class="window-bar-fill" style:width="{currentWindow.current}%"></div>
            </div>
            <!-- Time elapsed within the window -->
            <div class="window-time-bar">
              <div class="window-time-fill" style:width="{currentWindow.elapsedFraction * 100}%"></div>
            </div>
          </div>
          <div class="window-bar-legend">
            <span>0%</span>
            {#if proj > currentWindow.current}
              <span class="legend-proj-text">▬ projected end</span>
            {/if}
            <span>100%</span>
          </div>
        </div>
      {/if}

      <!-- Main chart -->
      {#if records.length >= 2}
        <div
          class="chart-card"
          bind:this={chartCard}
          role="img"
          aria-label="Usage chart"
          onmousemove={onMouseMove}
          onmouseleave={() => (tooltip = null)}
        >
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
                <span class="tt-dot dot-5h"></span><span>5-hour</span>
                <strong>{fmtPct(tooltip.record.five_hour_pct)}</strong>
              </div>
              <div class="tt-row">
                <span class="tt-dot dot-7d"></span><span>7-day</span>
                <strong>{fmtPct(tooltip.record.seven_day_pct)}</strong>
              </div>
              {#if tooltip.record.five_hour_predicted_pct != null}
                <div class="tt-row">
                  <span class="tt-dot dot-pred"></span><span>predicted</span>
                  <strong>{fmtPct(tooltip.record.five_hour_predicted_pct)}</strong>
                </div>
              {/if}
            </div>
          {/if}
          <svg viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}>
            {#if chart}
              {#each chart.grid as g}
                <line x1={PAD_L} x2={CHART_WIDTH - PAD_R} y1={g.y} y2={g.y} class="grid-line" />
                <text x={PAD_L - 4} y={g.y + 4} class="grid-label" text-anchor="end">{g.label}</text>
              {/each}
              {#each chart.resets as rx}
                <line x1={rx} x2={rx} y1={PAD_T} y2={PAD_T + chart.iH} class="reset-line" />
              {/each}
              <path d={chart.area5h} class="area-fill-5h" />
              <path d={chart.line7d} class="line-7d" />
              <path d={chart.line5h} class="line-5h" />
              {#if chart.predLine}
                <path d={chart.predLine} class="line-pred" />
              {/if}
              {#if tooltip}
                <line x1={tooltip.svgX} x2={tooltip.svgX} y1={PAD_T} y2={PAD_T + chart.iH} class="crosshair" />
                <circle cx={tooltip.svgX} cy={chart.toY(tooltip.record.five_hour_pct)} r="3.5" class="dot-circle-5h" />
                <circle cx={tooltip.svgX} cy={chart.toY(tooltip.record.seven_day_pct)} r="3" class="dot-circle-7d" />
              {/if}
              {#each chart.ticks as tick}
                <text x={tick.x} y={CHART_HEIGHT - 3} class="time-label" text-anchor="middle">{tick.label}</text>
              {/each}
            {/if}
          </svg>
          <div class="chart-legend">
            <span class="legend-item"><span class="legend-dot" style="background: hsl(var(--chart-5h))"></span>5-hour</span>
            <span class="legend-item"><span class="legend-dot" style="background: hsl(var(--chart-7d))"></span>7-day</span>
            {#if chart?.predLine}
              <span class="legend-item"><span class="legend-dash"></span>predicted</span>
            {/if}
            {#if chart && chart.resets.length > 0}
              <span class="legend-item"><span class="legend-reset"></span>window reset</span>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Stats row -->
      {#if stats}
        <div class="stats-row">
          <div class="stat"><span class="stat-label">Low 5h</span><span class="stat-value">{fmtPct(stats.min5h)}</span></div>
          <div class="stat"><span class="stat-label">Max 5h</span><span class="stat-value">{fmtPct(stats.max5h)}</span></div>
          <div class="stat"><span class="stat-label">Avg 5h</span><span class="stat-value">{fmtPct(stats.avg5h)}</span></div>
          <div class="stat"><span class="stat-label">Max 7d</span><span class="stat-value">{fmtPct(stats.max7d)}</span></div>
          <div class="stat"><span class="stat-label">Polls</span><span class="stat-value">{stats.count}</span></div>
        </div>
      {/if}

      <!-- ── Adaptive insight panels ─────────────────────────────────────────── -->

      <!-- 5h view: nothing extra (chart + burndown card is enough) -->

      <!-- 24h / 7d view: weekly pace + daily pattern -->
      {#if (displayRange === "24h" || displayRange === "7d") && pace}
        <section class="section">
          <h2>Weekly pace</h2>
          <p class="section-sub">
            Compares your current 7-day usage against spending the budget evenly until it resets.
          </p>
          <div class="pace-card pace-{pace.verdict}">
            <div class="pace-head">
              <span class="pace-icon" aria-hidden="true">
                {pace.verdict === "spare" ? "🟢" : pace.verdict === "ease" ? "🔴" : "🟡"}
              </span>
              <div>
                <div class="pace-title">{PACE_COPY[pace.verdict].title}</div>
                <div class="pace-tone">{PACE_COPY[pace.verdict].tone}</div>
              </div>
            </div>
            <div
              class="pace-bar"
              role="img"
              aria-label={`${fmtPct(pace.current)} used, even pace at ${fmtPct(pace.evenPacePct)}`}
            >
              <div class="pace-fill" style:width="{Math.min(pace.current, 100)}%"></div>
              <div class="pace-marker" style:left="{Math.min(pace.evenPacePct, 100)}%">
                <span class="pace-marker-label">even pace</span>
              </div>
            </div>
            <div class="pace-scale"><span>0%</span><span>weekly limit · 100%</span></div>
            <div class="pace-stats">
              <div class="pace-stat">
                <span class="pace-stat-label">Used now</span>
                <span class="pace-stat-value">{fmtPct(pace.current)}</span>
              </div>
              <div class="pace-stat">
                <span class="pace-stat-label">vs. even pace</span>
                <span
                  class="pace-stat-value"
                  class:over={pace.paceDiff > 10}
                  class:under={pace.paceDiff < -10}
                >
                  {pace.paceDiff >= 0 ? "+" : "−"}{Math.abs(pace.paceDiff).toFixed(0)}%
                </span>
              </div>
              <div class="pace-stat">
                <span class="pace-stat-label">Safe daily budget</span>
                <span class="pace-stat-value">{fmtRate(pace.safePerDay)}</span>
              </div>
              {#if pace.burnPerDay != null}
                <div class="pace-stat">
                  <span class="pace-stat-label">Recent burn</span>
                  <span
                    class="pace-stat-value"
                    class:over={pace.burnPerDay > pace.safePerDay + 1}
                    class:under={pace.burnPerDay < pace.safePerDay - 1}
                  >
                    {fmtRate(pace.burnPerDay)}
                  </span>
                </div>
              {/if}
              <div class="pace-stat">
                <span class="pace-stat-label">Resets</span>
                <span class="pace-stat-value">{fmtResetDate(pace.resetAt)} · {fmtDays(pace.daysToReset)}</span>
              </div>
            </div>
          </div>
        </section>
      {/if}

      {#if (displayRange === "24h" || displayRange === "7d") && records.length >= 24}
        <section class="section">
          <h2>Daily pattern</h2>
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

      <!-- 14d / 21d view: week-over-week + heatmap -->
      {#if (displayRange === "14d" || displayRange === "21d") && weekComparison.length > 0}
        <section class="section">
          <h2>Week over week</h2>
          <p class="section-sub">Daily 7-day budget consumed per week</p>
          <div class="week-compare">
            {#each weekComparison as week, wi}
              <div class="week-row">
                <div class="week-label">{week.label}</div>
                <div class="week-days">
                  {#each week.days as day}
                    <div class="week-day-col">
                      <div class="week-day-bar-wrap">
                        <div
                          class="week-day-bar"
                          class:week-day-heavy={day.consumption >= maxWeekDay * 0.7}
                          style:height="{(day.consumption / maxWeekDay) * 100}%"
                          title="{day.label}: {day.consumption.toFixed(1)}%"
                        ></div>
                      </div>
                      <div class="week-day-label">{day.label}</div>
                    </div>
                  {/each}
                </div>
                <div class="week-total">{week.total.toFixed(1)}%</div>
              </div>
            {/each}
          </div>
        </section>
      {/if}

      {#if (displayRange === "14d" || displayRange === "21d") && records.length >= 48}
        <section class="section">
          <h2>Usage heatmap</h2>
          <p class="section-sub">Average 5-hour usage by day of week and hour (local time)</p>
          <div
            class="heatmap"
            bind:this={heatmapCard}
            role="img"
            aria-label="Usage heatmap by day and hour"
            onmouseleave={() => (heatTooltip = null)}
          >
            {#if heatTooltip}
              <div
                class="tooltip heat-tooltip"
                style:left={heatTooltip.domX < (heatmapCard?.clientWidth ?? 0) / 2
                  ? `${heatTooltip.domX + 14}px`
                  : "auto"}
                style:right={heatTooltip.domX >= (heatmapCard?.clientWidth ?? 0) / 2
                  ? `${(heatmapCard?.clientWidth ?? 0) - heatTooltip.domX + 14}px`
                  : "auto"}
                style:top={`${Math.max(4, heatTooltip.domY - 46)}px`}
              >
                <div class="tt-time">{heatTooltip.label}</div>
                <div class="tt-row">
                  <span class="tt-dot dot-5h"></span><span>avg 5-hour</span>
                  <strong>{fmtPct(heatTooltip.val)}</strong>
                </div>
              </div>
            {/if}
            <!-- Hour header -->
            <div class="heatmap-row heatmap-header">
              <div class="heatmap-dow-label"></div>
              {#each Array(24) as _, h}
                <div class="heatmap-hour-label">{h % 6 === 0 ? hourLabel(h) : ""}</div>
              {/each}
            </div>
            {#each heatmap as row, dow}
              <div class="heatmap-row">
                <div class="heatmap-dow-label">{DOW_LABELS[dow]}</div>
                {#each row as val, h}
                  <div
                    class="heatmap-cell"
                    class:heatmap-cell-active={heatTooltip?.label === `${DOW_LABELS[dow]} · ${hourLabel(h)}`}
                    style:background={val != null
                      ? (theme.isDark
                          ? `hsl(222 80% ${Math.round(20 + (val / maxHeatmapCell) * 55)}%)`
                          : `hsl(222 84% ${Math.round(95 - (val / maxHeatmapCell) * 55)}%)`)
                      : (theme.isDark ? "hsl(217 32% 13%)" : "hsl(214 20% 95%)")}
                    role="presentation"
                    onmousemove={(e) => onHeatMove(e, dow, h, val)}
                  ></div>
                {/each}
              </div>
            {/each}
            <!-- Color scale -->
            <div class="heatmap-scale">
              <span>low</span>
              <div class="heatmap-scale-bar"></div>
              <span>high</span>
            </div>
          </div>
        </section>
      {/if}

    {:else}
      <!-- ── Resets panel ─────────────────────────────────────────────────────── -->
      <section class="section">
        <div class="resets-header">
          <div>
            <h2>Reset log</h2>
            <p class="section-sub">
              When each usage window reset, and how much you'd used before it did.
            </p>
          </div>
        </div>

        {#if resets.length === 0}
          <div class="resets-empty">
            <p>No resets detected in the last {sinceDays === 1 ? "24 hours" : `${sinceDays} days`}.</p>
            <p class="sub">Try a longer range, or wait for more polling cycles.</p>
          </div>
        {:else}
          <!-- Summary tiles -->
          <div class="reset-tiles">
            <div class="rtile">
              <span class="rtile-value">{resetSummary.count5h}</span>
              <span class="rtile-label">5-hour resets</span>
            </div>
            <div class="rtile" class:rtile-hot={resetSummary.maxed > 0}>
              <span class="rtile-value">{resetSummary.maxed}</span>
              <span class="rtile-label">maxed out</span>
            </div>
            <div class="rtile">
              <span class="rtile-value">{fmtPct(resetSummary.avgPeak)}</span>
              <span class="rtile-label">avg peak</span>
            </div>
            <div class="rtile">
              <span class="rtile-value">{resetSummary.count7d}</span>
              <span class="rtile-label">weekly resets</span>
            </div>
          </div>

          <!-- Timeline: each row is one window that reset, with its peak usage -->
          <div class="reset-timeline">
            {#each resetsByDay as day (day.key)}
              <div class="rt-day">
                <div class="rt-day-label">{day.label}</div>
                <div class="rt-rows">
                  {#each day.events as e (e.windowType + e.resetAt)}
                    {#if e.windowType === "7d"}
                      <div class="rt-row rt-row-7d">
                        <span class="rt-time">{fmtClock(e.resetAt)}</span>
                        <span class="rt-badge badge-7d">7-day</span>
                        <span class="rt-desc">
                          Weekly limit reset — peaked at {fmtPct(e.peakPct)}
                        </span>
                      </div>
                    {:else}
                      <div class="rt-row" class:rt-maxed={e.hitCap}>
                        <span class="rt-time">{fmtClock(e.resetAt)}</span>
                        <div
                          class="rt-bar"
                          role="img"
                          aria-label={`peaked at ${fmtPct(e.peakPct)}`}
                        >
                          <div
                            class="rt-bar-fill"
                            class:rt-bar-high={e.peakPct >= 80}
                            style:width="{Math.min(e.peakPct, 100)}%"
                          ></div>
                        </div>
                        <span class="rt-peak">{fmtPct(e.peakPct)}</span>
                        {#if e.hitCap}
                          <span class="rt-badge badge-max">maxed</span>
                        {:else}
                          <span class="rt-badge-spacer"></span>
                        {/if}
                      </div>
                    {/if}
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {/if}
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
    color: hsl(var(--foreground));
  }

  .subtitle {
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
    margin: 0;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  /* ── View tabs ───────────────────────────────────────────────────────────────── */
  .view-tabs {
    display: flex;
    gap: 0.25rem;
    border: 1px solid hsl(var(--border));
    border-radius: 6px;
    padding: 2px;
    background: hsl(var(--muted));
  }

  .view-tab {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.3rem 0.75rem;
    background: none;
    border: none;
    border-radius: 4px;
    font-size: 0.8125rem;
    color: hsl(var(--muted-foreground));
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }

  .view-tab.active {
    background: hsl(var(--card));
    color: hsl(var(--foreground));
    font-weight: 600;
    box-shadow: 0 1px 3px hsl(222.2 84% 4.9% / 0.08);
  }

  /* ── Instance tabs ──────────────────────────────────────────────────────────── */
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

  .tab.active {
    background: hsl(var(--card));
    color: hsl(var(--foreground));
    font-weight: 600;
    box-shadow: 0 1px 3px hsl(222.2 84% 4.9% / 0.08);
  }

  /* ── Range selector ─────────────────────────────────────────────────────────── */
  .range-selector {
    display: flex;
    border: 1px solid hsl(var(--border));
    border-radius: 6px;
    overflow: hidden;
  }

  .range-btn {
    padding: 0.3rem 0.6rem;
    background: none;
    border: none;
    border-right: 1px solid hsl(var(--border));
    font-size: 0.8125rem;
    color: hsl(var(--muted-foreground));
    cursor: pointer;
  }

  .range-btn:last-child {
    border-right: none;
  }

  .range-btn.active {
    background: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
  }

  /* ── Error / loading / empty ────────────────────────────────────────────────── */
  .error-banner {
    padding: 0.75rem 1rem;
    background: hsl(var(--danger-bg));
    color: hsl(var(--danger-strong));
    border-radius: 8px;
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding: 3rem 0;
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
  }

  .spinner {
    width: 28px;
    height: 28px;
    border: 3px solid hsl(var(--border));
    border-top-color: hsl(var(--foreground));
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .empty-state {
    padding: 3rem 1rem;
    text-align: center;
    color: hsl(var(--muted-foreground));
  }

  .empty-state p { margin: 0 0 0.5rem; font-size: 0.9rem; }
  .empty-state .sub { font-size: 0.8rem; }

  /* ── Prediction card ────────────────────────────────────────────────────────── */
  .prediction-card {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 1rem 1.25rem;
    border: 1px solid hsl(var(--border));
    border-radius: 8px;
    background: hsl(var(--card));
    margin-bottom: 1rem;
  }

  .prediction-card.warn {
    border-color: hsl(var(--warning-border));
    background: hsl(var(--warning-bg));
  }

  .prediction-card.danger {
    border-color: hsl(var(--danger-border));
    background: hsl(var(--danger-bg));
  }

  .pred-main {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
  }

  .pred-number {
    font-size: 2rem;
    font-weight: 700;
    color: hsl(var(--foreground));
    line-height: 1;
  }

  .prediction-card.warn .pred-number { color: hsl(var(--warning-strong)); }
  .prediction-card.danger .pred-number { color: hsl(var(--danger-strong)); }

  .pred-label {
    font-size: 0.875rem;
    color: hsl(var(--muted-foreground));
  }

  .pred-meta {
    display: flex;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: hsl(var(--muted-foreground));
    flex-wrap: wrap;
  }

  .pred-source.empirical { color: hsl(var(--success-strong)); }

  /* ── 5h window burndown card ─────────────────────────────────────────────────── */
  .window-card {
    padding: 1rem 1.25rem;
    border: 1px solid hsl(var(--border));
    border-radius: 8px;
    background: hsl(var(--card));
    margin-bottom: 1rem;
  }

  .window-card.window-warn {
    border-color: hsl(var(--warning-border));
    background: hsl(var(--warning-bg));
  }

  .window-card.window-danger {
    border-color: hsl(var(--danger-border));
    background: hsl(var(--danger-bg));
  }

  .window-stats {
    display: flex;
    gap: 2rem;
    margin-bottom: 0.875rem;
  }

  .window-stat {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .window-stat-label {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: hsl(var(--muted-foreground));
  }

  .window-stat-value {
    font-size: 1.25rem;
    font-weight: 700;
    color: hsl(var(--foreground));
    font-variant-numeric: tabular-nums;
  }

  .window-bar-wrap {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .window-bar {
    position: relative;
    height: 14px;
    background: hsl(var(--chart-bar-track));
    border-radius: 7px;
    overflow: hidden;
  }

  .window-bar-proj {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    background: hsl(var(--chart-5h-soft));
    border-radius: 7px;
  }

  .window-bar-fill {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    background: hsl(var(--chart-5h));
    border-radius: 7px;
  }

  .window-card.window-warn .window-bar-fill { background: hsl(var(--warning)); }
  .window-card.window-danger .window-bar-fill { background: hsl(var(--danger)); }

  .window-time-bar {
    height: 4px;
    background: hsl(var(--chart-bar-track));
    border-radius: 2px;
    overflow: hidden;
  }

  .window-time-fill {
    height: 100%;
    background: hsl(var(--muted-foreground));
    border-radius: 2px;
  }

  .window-bar-legend {
    display: flex;
    justify-content: space-between;
    font-size: 0.65rem;
    color: hsl(var(--muted-foreground));
    margin-top: 0.2rem;
  }

  .legend-proj-text {
    color: hsl(var(--chart-5h-soft));
  }

  /* ── Chart card ─────────────────────────────────────────────────────────────── */
  .chart-card {
    position: relative;
    border: 1px solid hsl(var(--border));
    border-radius: 8px;
    padding: 0.75rem 0.75rem 0.4rem;
    background: hsl(var(--card));
    margin-bottom: 1rem;
    cursor: crosshair;
    user-select: none;
  }

  svg {
    width: 100%;
    height: auto;
    display: block;
  }

  .grid-line { stroke: hsl(var(--chart-grid)); stroke-width: 1; }
  .grid-label { font-size: 9px; fill: hsl(var(--chart-axis)); font-family: inherit; }
  .time-label { font-size: 8px; fill: hsl(var(--chart-axis)); font-family: inherit; }
  .reset-line { stroke: hsl(var(--chart-reset)); stroke-width: 1; stroke-dasharray: 4 3; }
  .area-fill-5h { fill: hsl(var(--chart-5h) / 0.12); }
  .line-5h { fill: none; stroke: hsl(var(--chart-5h)); stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
  .line-7d { fill: none; stroke: hsl(var(--chart-7d)); stroke-width: 1.5; stroke-linejoin: round; stroke-linecap: round; }
  .line-pred { fill: none; stroke: hsl(var(--chart-pred)); stroke-width: 1.5; stroke-dasharray: 5 3; opacity: 0.5; }
  .crosshair { stroke: hsl(var(--chart-axis)); stroke-width: 1; stroke-dasharray: 3 2; pointer-events: none; }
  .dot-circle-5h { fill: hsl(var(--chart-5h)); pointer-events: none; }
  .dot-circle-7d { fill: hsl(var(--chart-7d)); pointer-events: none; }

  .chart-legend {
    display: flex;
    gap: 1rem;
    padding: 0.35rem 0.25rem 0;
    font-size: 0.75rem;
    color: hsl(var(--muted-foreground));
  }

  .legend-item { display: flex; align-items: center; gap: 0.35rem; }
  .legend-dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .legend-dash { width: 18px; height: 0; border-top: 2px dashed hsl(var(--chart-pred)); opacity: 0.55; flex-shrink: 0; }
  .legend-reset { width: 18px; height: 0; border-top: 1px dashed hsl(var(--chart-reset)); flex-shrink: 0; }

  /* ── Tooltip ────────────────────────────────────────────────────────────────── */
  .tooltip {
    position: absolute;
    pointer-events: none;
    z-index: 10;
    background: hsl(var(--tooltip-bg));
    color: hsl(var(--tooltip-fg));
    border-radius: 6px;
    padding: 0.5rem 0.7rem;
    font-size: 0.75rem;
    min-width: 158px;
    box-shadow: 0 4px 14px hsl(0 0% 0% / 0.3);
  }

  .tt-time { font-size: 0.7rem; opacity: 0.6; margin-bottom: 0.4rem; }
  .tt-row { display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.18rem; color: hsl(var(--tooltip-muted)); }
  .tt-row:last-child { margin-bottom: 0; }
  .tt-row strong { margin-left: auto; color: hsl(var(--tooltip-fg)); font-weight: 600; }
  .tt-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .dot-5h { background: hsl(222 84% 65%); }
  .dot-7d { background: hsl(25 95% 62%); }
  .dot-pred { background: hsl(222 84% 65%); opacity: 0.55; }

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
    border: 1px solid hsl(var(--border));
    border-radius: 8px;
    background: hsl(var(--card));
  }

  .stat-label { font-size: 0.7rem; color: hsl(var(--muted-foreground)); text-transform: uppercase; letter-spacing: 0.04em; }
  .stat-value { font-size: 1.1rem; font-weight: 600; color: hsl(var(--foreground)); }

  /* ── Section ────────────────────────────────────────────────────────────────── */
  .section { margin-bottom: 2rem; }
  h2 { font-size: 0.9375rem; font-weight: 600; color: hsl(var(--foreground)); margin: 0 0 0.2rem; }
  .section-sub { font-size: 0.8rem; color: hsl(var(--muted-foreground)); margin: 0 0 0.75rem; }

  /* ── Weekly pace ────────────────────────────────────────────────────────────── */
  .pace-card {
    border: 1px solid hsl(var(--border));
    border-radius: 8px;
    background: hsl(var(--card));
    padding: 1.1rem 1.25rem 1.25rem;
  }

  .pace-card.pace-spare { border-color: hsl(var(--success-border)); background: hsl(var(--success-bg)); }
  .pace-card.pace-ease { border-color: hsl(var(--danger-border)); background: hsl(var(--danger-bg)); }

  .pace-head { display: flex; gap: 0.6rem; align-items: flex-start; margin-bottom: 1rem; }
  .pace-icon { font-size: 1.1rem; line-height: 1.4; }
  .pace-title { font-size: 0.95rem; font-weight: 600; color: hsl(var(--foreground)); }
  .pace-tone { font-size: 0.8rem; color: hsl(var(--muted-foreground)); margin-top: 0.1rem; }

  .pace-bar {
    position: relative;
    height: 12px;
    border-radius: 6px;
    background: hsl(var(--chart-bar-track));
    overflow: visible;
    margin-top: 0.5rem;
  }

  .pace-fill { height: 100%; border-radius: 6px; background: hsl(var(--chart-7d)); }
  .pace-spare .pace-fill { background: hsl(var(--success)); }
  .pace-ease .pace-fill { background: hsl(var(--danger)); }

  .pace-marker {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 2px;
    background: hsl(var(--foreground));
    transform: translateX(-1px);
  }

  .pace-marker-label {
    position: absolute;
    top: -16px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 0.6rem;
    color: hsl(var(--foreground));
    white-space: nowrap;
  }

  .pace-scale { display: flex; justify-content: space-between; font-size: 0.65rem; color: hsl(var(--muted-foreground)); margin-top: 0.35rem; }

  .pace-stats {
    display: flex;
    flex-wrap: wrap;
    gap: 1.25rem;
    margin-top: 1.1rem;
    padding-top: 1rem;
    border-top: 1px solid hsl(var(--border));
  }

  .pace-stat { display: flex; flex-direction: column; gap: 0.15rem; }
  .pace-stat-label { font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.04em; color: hsl(var(--muted-foreground)); }
  .pace-stat-value { font-size: 0.95rem; font-weight: 600; color: hsl(var(--foreground)); font-variant-numeric: tabular-nums; }
  .pace-stat-value.over { color: hsl(var(--danger-strong)); }
  .pace-stat-value.under { color: hsl(var(--success-strong)); }

  /* ── Daily pattern ──────────────────────────────────────────────────────────── */
  .daily-chart { display: flex; gap: 2px; height: 96px; }
  .hour-col { flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .bar-track { flex: 1; background: hsl(var(--chart-bar-track)); border-radius: 2px 2px 0 0; display: flex; align-items: flex-end; overflow: hidden; }
  .bar-fill { width: 100%; background: hsl(var(--chart-bar)); border-radius: 2px 2px 0 0; min-height: 1px; }
  .bar-high { background: hsl(var(--danger)); }
  .bar-mid { background: hsl(var(--warning)); }
  .hour-label { font-size: 0.6rem; color: hsl(var(--muted-foreground)); text-align: center; height: 14px; line-height: 14px; white-space: nowrap; }

  /* ── Week-over-week ─────────────────────────────────────────────────────────── */
  .week-compare {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .week-row {
    display: grid;
    grid-template-columns: 140px 1fr 52px;
    align-items: end;
    gap: 0.75rem;
  }

  .week-label {
    font-size: 0.75rem;
    color: hsl(var(--muted-foreground));
    padding-bottom: 16px;
  }

  .week-days {
    display: flex;
    gap: 4px;
    align-items: flex-end;
    height: 64px;
  }

  .week-day-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    height: 100%;
  }

  .week-day-bar-wrap {
    flex: 1;
    background: hsl(var(--chart-bar-track));
    border-radius: 2px 2px 0 0;
    display: flex;
    align-items: flex-end;
    overflow: hidden;
  }

  .week-day-bar {
    width: 100%;
    background: hsl(var(--chart-bar));
    border-radius: 2px 2px 0 0;
    min-height: 1px;
  }

  .week-day-heavy { background: hsl(var(--danger)); }

  .week-day-label {
    font-size: 0.58rem;
    color: hsl(var(--muted-foreground));
    text-align: center;
    height: 14px;
    line-height: 14px;
  }

  .week-total {
    font-size: 0.8rem;
    font-weight: 600;
    color: hsl(var(--foreground));
    font-variant-numeric: tabular-nums;
    text-align: right;
    padding-bottom: 16px;
  }

  /* ── Heatmap ────────────────────────────────────────────────────────────────── */
  .heatmap {
    position: relative;
    border: 1px solid hsl(var(--border));
    border-radius: 8px;
    padding: 0.5rem;
    background: hsl(var(--card));
  }

  .heat-tooltip { min-width: 0; }

  .heatmap-row {
    display: grid;
    grid-template-columns: 28px repeat(24, 1fr);
    gap: 2px;
    margin-bottom: 2px;
  }

  .heatmap-row:last-child { margin-bottom: 0; }

  .heatmap-header { margin-bottom: 4px; }

  .heatmap-dow-label {
    font-size: 0.6rem;
    color: hsl(var(--muted-foreground));
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-right: 4px;
  }

  .heatmap-hour-label {
    font-size: 0.55rem;
    color: hsl(var(--muted-foreground));
    text-align: center;
    height: 12px;
    line-height: 12px;
  }

  .heatmap-cell {
    height: 14px;
    border-radius: 2px;
    transition: opacity 0.1s;
  }

  .heatmap-cell:hover { opacity: 0.75; }

  .heatmap-cell-active {
    box-shadow: inset 0 0 0 1.5px hsl(var(--foreground));
  }

  .heatmap-scale {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.5rem;
    padding-top: 0.4rem;
    border-top: 1px solid hsl(var(--border));
    font-size: 0.65rem;
    color: hsl(var(--muted-foreground));
  }

  .heatmap-scale-bar {
    flex: 1;
    height: 8px;
    border-radius: 4px;
    background: linear-gradient(to right, hsl(var(--heat-lo)), hsl(var(--heat-hi)));
  }

  /* ── Reset log ──────────────────────────────────────────────────────────────── */
  .resets-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .resets-empty { padding: 2rem; text-align: center; color: hsl(var(--muted-foreground)); font-size: 0.875rem; }
  .resets-empty .sub { font-size: 0.8rem; margin-top: 0.3rem; }

  /* Summary tiles */
  .reset-tiles {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.75rem;
    margin-bottom: 1.5rem;
  }

  .rtile {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.7rem 0.85rem;
    border: 1px solid hsl(var(--border));
    border-radius: 8px;
    background: hsl(var(--card));
  }

  .rtile-hot { border-color: hsl(var(--danger-border)); background: hsl(var(--danger-bg)); }
  .rtile-value { font-size: 1.4rem; font-weight: 700; color: hsl(var(--foreground)); font-variant-numeric: tabular-nums; line-height: 1; }
  .rtile-hot .rtile-value { color: hsl(var(--danger-strong)); }
  .rtile-label { font-size: 0.7rem; color: hsl(var(--muted-foreground)); text-transform: uppercase; letter-spacing: 0.04em; }

  /* Timeline */
  .reset-timeline { display: flex; flex-direction: column; gap: 1.25rem; }

  .rt-day {
    display: grid;
    grid-template-columns: 108px 1fr;
    gap: 0.75rem;
    align-items: start;
  }

  .rt-day-label {
    font-size: 0.78rem;
    font-weight: 600;
    color: hsl(var(--muted-foreground));
    padding-top: 0.35rem;
    position: sticky;
    top: 0;
  }

  .rt-rows { display: flex; flex-direction: column; gap: 0.3rem; }

  .rt-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.35rem 0.6rem;
    border: 1px solid hsl(var(--border));
    border-radius: 7px;
    background: hsl(var(--card));
  }

  .rt-row.rt-maxed { border-color: hsl(var(--danger-border)); background: hsl(var(--danger-bg)); }

  .rt-time {
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    color: hsl(var(--muted-foreground));
    width: 52px;
    flex-shrink: 0;
  }

  .rt-bar {
    flex: 1;
    height: 8px;
    background: hsl(var(--chart-bar-track));
    border-radius: 4px;
    overflow: hidden;
  }

  .rt-bar-fill {
    height: 100%;
    background: hsl(var(--chart-5h));
    border-radius: 4px;
    min-width: 2px;
  }

  .rt-bar-high { background: hsl(var(--danger)); }

  .rt-peak {
    font-size: 0.82rem;
    font-weight: 600;
    color: hsl(var(--foreground));
    font-variant-numeric: tabular-nums;
    width: 40px;
    text-align: right;
    flex-shrink: 0;
  }

  .rt-badge {
    font-size: 0.62rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .badge-max { background: hsl(var(--danger)); color: white; }
  .badge-7d { background: hsl(var(--accent-purple)); color: white; }
  .rt-badge-spacer { width: 44px; flex-shrink: 0; }

  /* 7-day reset row spans full width and stands out */
  .rt-row-7d { border-color: hsl(var(--accent-purple)); background: hsl(var(--accent-purple) / 0.08); }
  .rt-desc { font-size: 0.8rem; color: hsl(var(--foreground)); }
</style>
