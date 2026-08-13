<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Switch from "../lib/Switch.svelte";
  import { setTheme, theme, type ThemePref } from "../lib/theme.svelte";
  import {
    setHighContrast,
    setReduceMotion,
    setStatusPalette,
    visuals,
    type StatusPalette,
  } from "../lib/visuals.svelte";

  type Config = {
    general: { poll_interval_seconds: number; timezone: string; show_tray_icon: boolean; start_minimized: boolean; show_pace_delta: boolean };
    thresholds: { five_hour: number[]; seven_day: number[] };
    notifications: {
      sound: boolean;
      sound_name: string;
      persistent: boolean;
      notify_threshold_crossings: boolean;
      notify_predicted_overage: boolean;
      notify_resets: boolean;
      minutes_before_five_hour_reset: number | null;
      minutes_before_seven_day_reset: number | null;
      capacity_warning_five_hour: [number, number] | null;
      capacity_warning_seven_day: [number, number] | null;
    };
    instances: { name: string }[];
    history: { enabled: boolean; max_records: number };
    web: { enabled: boolean; bind: string; port: number; agent_api_enabled: boolean };
  };

  let config = $state<Config | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let activeTab = $state<"general" | "visuals" | "notifications" | "thresholds" | "capacity" | "web">("general");

  // Form-bound values
  let pollIntervalMinutes = $state(15);
  let timezone = $state("local");
  let trayIconEnabled = $state(false);
  let startMinimized = $state(false);
  let showPaceDelta = $state(false);

  // Polling more often than this hammers Claude's servers without giving more
  // accurate readings, so we surface a warning below the field.
  const MIN_RECOMMENDED_INTERVAL = 5;
  const MAX_THRESHOLDS = 5;
  let fiveHourThresholds = $state<number[]>([50, 75, 90, 100]);
  let sevenDayThresholds = $state<number[]>([50, 75, 90, 100]);
  let fiveHourNewThreshold = $state("");
  let sevenDayNewThreshold = $state("");

  const SOUND_OPTIONS = [
    "Basso", "Blow", "Bottle", "Frog", "Funk", "Glass", "Hero",
    "Morse", "Ping", "Pop", "Purr", "Sosumi", "Submarine", "Tink",
  ];

  const THEME_OPTIONS: { preference: ThemePref; label: string }[] = [
    { preference: "system", label: "System" },
    { preference: "light", label: "Light" },
    { preference: "dark", label: "Dark" },
    { preference: "super-dark", label: "Super dark" },
  ];

  let soundEnabled = $state(true);
  let soundName = $state("Glass");
  let persistentEnabled = $state(false);
  let notifyThresholdCrossings = $state(true);
  let notifyPredictedOverage = $state(true);
  let notifyResets = $state(true);
  let minutesBeforeFiveHourReset = $state<string>("");
  let minutesBeforeSevenDayReset = $state<string>("");

  let capacityWarningFiveHourMinutes = $state<string>("");
  let capacityWarningFiveHourPercent = $state<string>("");
  let capacityWarningSevenDayMinutes = $state<string>("");
  let capacityWarningSevenDayPercent = $state<string>("");

  let historyEnabled = $state(false);
  let historyDays = $state(14);

  let webEnabled = $state(false);
  let webBind = $state("127.0.0.1");
  let webPort = $state(4317);
  let agentApiEnabled = $state(false);

  const BYTES_PER_RECORD = 200;

  function recordsFromDays(days: number): number {
    return Math.max(1, Math.round(days * ((24 * 60) / pollIntervalMinutes)));
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  let autostartEnabled = $state(false);
  let autostartError = $state<string | null>(null);

  let testingNotification = $state(false);

  type ToastKind = "success" | "error";
  let toast = $state<{ message: string; kind: ToastKind } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  function showToast(message: string, kind: ToastKind = "success") {
    toast = { message, kind };
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toast = null;
    }, 2500);
  }

  async function loadConfig() {
    loading = true;
    loadError = null;
    try {
      config = await invoke<Config>("get_config", {});

      pollIntervalMinutes = Math.round(config.general.poll_interval_seconds / 60);
      timezone = config.general.timezone;
      trayIconEnabled = config.general.show_tray_icon;
      startMinimized = config.general.start_minimized;
      showPaceDelta = config.general.show_pace_delta;
      fiveHourThresholds = [...config.thresholds.five_hour].sort((a, b) => a - b);
      sevenDayThresholds = [...config.thresholds.seven_day].sort((a, b) => a - b);

      soundEnabled = config.notifications.sound;
      soundName = config.notifications.sound_name;
      persistentEnabled = config.notifications.persistent;
      notifyThresholdCrossings = config.notifications.notify_threshold_crossings;
      notifyPredictedOverage = config.notifications.notify_predicted_overage;
      notifyResets = config.notifications.notify_resets;
      minutesBeforeFiveHourReset = config.notifications.minutes_before_five_hour_reset?.toString() ?? "";
      minutesBeforeSevenDayReset = config.notifications.minutes_before_seven_day_reset?.toString() ?? "";

      if (config.notifications.capacity_warning_five_hour) {
        capacityWarningFiveHourMinutes = config.notifications.capacity_warning_five_hour[0].toString();
        capacityWarningFiveHourPercent = config.notifications.capacity_warning_five_hour[1].toString();
      }
      if (config.notifications.capacity_warning_seven_day) {
        capacityWarningSevenDayMinutes = config.notifications.capacity_warning_seven_day[0].toString();
        capacityWarningSevenDayPercent = config.notifications.capacity_warning_seven_day[1].toString();
      }

      historyEnabled = config.history.enabled;
      historyDays = Math.max(1, Math.round(config.history.max_records / ((24 * 60) / Math.round(config.general.poll_interval_seconds / 60))));

      webEnabled = config.web.enabled;
      webBind = config.web.bind;
      webPort = config.web.port;
      agentApiEnabled = config.web.agent_api_enabled;
    } catch (e) {
      loadError = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadAutostart() {
    try {
      autostartEnabled = await invoke<boolean>("get_autostart");
    } catch (e) {
      autostartError = String(e);
    }
  }

  async function persist(mutator: (c: Config) => void, successMessage: string) {
    if (!config) return;
    const next: Config = JSON.parse(JSON.stringify(config));
    mutator(next);
    try {
      await invoke("set_config", { config: next });
      config = next;
      showToast(successMessage);
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  function savePollInterval() {
    const minutes = Math.max(1, Math.round(pollIntervalMinutes));
    pollIntervalMinutes = minutes;
    // History is stored as a record count, not a time span. Changing the poll
    // interval would silently change how many days that count covers, so we
    // recompute max_records to preserve the user's chosen retention in days.
    const newRecords = recordsFromDays(historyDays);
    const dayWord = historyDays === 1 ? "day" : "days";
    persist((c) => {
      c.general.poll_interval_seconds = minutes * 60;
      c.history.max_records = newRecords;
    }, historyEnabled
      ? `Interval set to ${minutes} min · history retention kept at ~${historyDays} ${dayWord} (${newRecords.toLocaleString()} records)`
      : "Polling interval updated");
  }

  function saveTimezone() {
    persist((c) => {
      c.general.timezone = timezone;
    }, "Timezone updated");
  }

  async function toggleTrayIcon() {
    trayIconEnabled = !trayIconEnabled;
    // Apply live first so the icon appears/disappears immediately, then persist.
    try {
      await invoke("set_tray_visible", { visible: trayIconEnabled });
    } catch (e) {
      showToast(String(e), "error");
    }
    persist((c) => {
      c.general.show_tray_icon = trayIconEnabled;
    }, trayIconEnabled ? "Menu bar icon enabled" : "Menu bar icon disabled");
  }

  function toggleStartMinimized() {
    startMinimized = !startMinimized;
    persist((c) => {
      c.general.start_minimized = startMinimized;
    }, startMinimized ? "Start minimized at login enabled" : "Start minimized at login disabled");
  }

  function toggleShowPaceDelta() {
    showPaceDelta = !showPaceDelta;
    persist((c) => {
      c.general.show_pace_delta = showPaceDelta;
    }, showPaceDelta ? "Pace delta shown on bars" : "Pace delta hidden");
  }

  function toggleReduceMotion() {
    setReduceMotion(!visuals.reduceMotion);
  }

  function toggleHighContrast() {
    setHighContrast(!visuals.highContrast);
  }

  type ThresholdLimit = "five_hour" | "seven_day";

  function thresholdsFor(limit: ThresholdLimit): number[] {
    return limit === "five_hour" ? fiveHourThresholds : sevenDayThresholds;
  }

  function setThresholdsFor(limit: ThresholdLimit, values: number[]) {
    if (limit === "five_hour") {
      fiveHourThresholds = values;
    } else {
      sevenDayThresholds = values;
    }
  }

  function persistThresholds(limit: ThresholdLimit, values: number[], message: string) {
    persist((c) => {
      if (limit === "five_hour") {
        c.thresholds.five_hour = values;
      } else {
        c.thresholds.seven_day = values;
      }
    }, message);
  }

  function addThreshold(limit: ThresholdLimit) {
    const raw = limit === "five_hour" ? fiveHourNewThreshold : sevenDayNewThreshold;
    const current = thresholdsFor(limit);
    const label = limit === "five_hour" ? "5-hour" : "7-day";

    const n = parseInt(raw, 10);
    if (raw.trim() === "" || isNaN(n) || n < 0 || n > 100) {
      showToast("Threshold must be a whole number between 0 and 100", "error");
      return;
    }
    if (current.length >= MAX_THRESHOLDS) {
      showToast(`A limit can have at most ${MAX_THRESHOLDS} thresholds`, "error");
      return;
    }
    if (current.includes(n)) {
      showToast(`${n}% is already in the ${label} list`, "error");
      return;
    }

    const next = [...current, n].sort((a, b) => a - b);
    setThresholdsFor(limit, next);
    if (limit === "five_hour") {
      fiveHourNewThreshold = "";
    } else {
      sevenDayNewThreshold = "";
    }
    persistThresholds(limit, next, `${label} thresholds updated`);
  }

  function removeThreshold(limit: ThresholdLimit, value: number) {
    const current = thresholdsFor(limit);
    const label = limit === "five_hour" ? "5-hour" : "7-day";
    if (current.length <= 1) {
      showToast("At least 1 threshold is required", "error");
      return;
    }
    const next = current.filter((v) => v !== value);
    setThresholdsFor(limit, next);
    persistThresholds(limit, next, `${label} thresholds updated`);
  }

  function toggleSound() {
    soundEnabled = !soundEnabled;
    persist((c) => {
      c.notifications.sound = soundEnabled;
    }, soundEnabled ? "Notification sounds enabled" : "Notification sounds disabled");
  }

  function saveSoundName() {
    persist((c) => {
      c.notifications.sound_name = soundName;
    }, "Notification sound updated");
  }

  function togglePersistent() {
    persistentEnabled = !persistentEnabled;
    persist((c) => {
      c.notifications.persistent = persistentEnabled;
    }, persistentEnabled ? "Persistent notifications enabled" : "Persistent notifications disabled");
  }

  function toggleNotifyThresholdCrossings() {
    notifyThresholdCrossings = !notifyThresholdCrossings;
    persist((c) => {
      c.notifications.notify_threshold_crossings = notifyThresholdCrossings;
    }, notifyThresholdCrossings ? "Threshold crossing alerts enabled" : "Threshold crossing alerts disabled");
  }

  function toggleNotifyPredictedOverage() {
    notifyPredictedOverage = !notifyPredictedOverage;
    persist((c) => {
      c.notifications.notify_predicted_overage = notifyPredictedOverage;
    }, notifyPredictedOverage ? "Predicted overage alerts enabled" : "Predicted overage alerts disabled");
  }

  function toggleNotifyResets() {
    notifyResets = !notifyResets;
    persist((c) => {
      c.notifications.notify_resets = notifyResets;
    }, notifyResets ? "Reset notifications enabled" : "Reset notifications disabled");
  }

  function saveMinutesBeforeFiveHourReset() {
    persist((c) => {
      c.notifications.minutes_before_five_hour_reset = minutesBeforeFiveHourReset ? parseInt(minutesBeforeFiveHourReset, 10) : null;
    }, "5-hour pre-reset alert updated");
  }

  function saveMinutesBeforeSevenDayReset() {
    persist((c) => {
      c.notifications.minutes_before_seven_day_reset = minutesBeforeSevenDayReset ? parseInt(minutesBeforeSevenDayReset, 10) : null;
    }, "7-day pre-reset alert updated");
  }

  function saveCapacityWarningFiveHour() {
    persist((c) => {
      if (capacityWarningFiveHourMinutes && capacityWarningFiveHourPercent) {
        const mins = parseInt(capacityWarningFiveHourMinutes, 10);
        const pct = parseInt(capacityWarningFiveHourPercent, 10);
        if (!isNaN(mins) && !isNaN(pct) && pct >= 0 && pct <= 100) {
          c.notifications.capacity_warning_five_hour = [mins, pct];
        } else {
          c.notifications.capacity_warning_five_hour = null;
        }
      } else {
        c.notifications.capacity_warning_five_hour = null;
      }
    }, "5-hour capacity warning updated");
  }

  function saveCapacityWarningSevenDay() {
    persist((c) => {
      if (capacityWarningSevenDayMinutes && capacityWarningSevenDayPercent) {
        const mins = parseInt(capacityWarningSevenDayMinutes, 10);
        const pct = parseInt(capacityWarningSevenDayPercent, 10);
        if (!isNaN(mins) && !isNaN(pct) && pct >= 0 && pct <= 100) {
          c.notifications.capacity_warning_seven_day = [mins, pct];
        } else {
          c.notifications.capacity_warning_seven_day = null;
        }
      } else {
        c.notifications.capacity_warning_seven_day = null;
      }
    }, "7-day capacity warning updated");
  }

  function toggleHistoryEnabled() {
    historyEnabled = !historyEnabled;
    persist((c) => {
      c.history.enabled = historyEnabled;
    }, historyEnabled ? "History recording enabled" : "History recording disabled");
  }

  function saveHistoryDays() {
    const days = Math.max(1, Math.round(historyDays));
    historyDays = days;
    persist((c) => {
      c.history.max_records = recordsFromDays(days);
    }, "History retention updated");
  }

  function toggleWebEnabled() {
    webEnabled = !webEnabled;
    persist((c) => {
      c.web.enabled = webEnabled;
    }, webEnabled ? `Web dashboard enabled at http://${webBind}:${webPort}` : "Web dashboard disabled");
  }

  function saveWebBind() {
    const bind = webBind.trim();
    if (!bind) {
      showToast("Bind address cannot be empty", "error");
      webBind = config?.web.bind ?? "127.0.0.1";
      return;
    }
    persist((c) => {
      c.web.bind = bind;
    }, "Web dashboard address updated");
  }

  function saveWebPort() {
    const port = Math.round(webPort);
    if (isNaN(port) || port < 1 || port > 65535) {
      showToast("Port must be between 1 and 65535", "error");
      webPort = config?.web.port ?? 4317;
      return;
    }
    webPort = port;
    persist((c) => {
      c.web.port = port;
    }, "Web dashboard port updated");
  }

  function toggleAgentApi() {
    agentApiEnabled = !agentApiEnabled;
    persist((c) => {
      c.web.agent_api_enabled = agentApiEnabled;
    }, agentApiEnabled ? "Agent API enabled at /api/agent" : "Agent API disabled");
  }

  async function toggleAutostart() {
    const next = !autostartEnabled;
    autostartError = null;
    try {
      await invoke("set_autostart", { enabled: next });
      autostartEnabled = next;
      showToast(next ? "Open at login enabled" : "Open at login disabled");
    } catch (e) {
      autostartError = String(e);
      showToast(String(e), "error");
    }
  }

  async function testNotification() {
    testingNotification = true;
    try {
      await invoke("test_notification", {});
      showToast("Test notification sent!");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      testingNotification = false;
    }
  }

  onMount(async () => {
    await Promise.all([loadConfig(), loadAutostart()]);
  });
</script>

<div class="page">
  <h1>Settings</h1>
  <p class="subtitle">Changes are applied immediately.</p>

  {#if loadError}
    <div class="error-banner" role="alert">⚠ {loadError}</div>
  {:else if loading}
    <div class="loading">
      <div class="spinner" aria-hidden="true"></div>
      <p>Loading settings…</p>
    </div>
  {:else if config}
    <div class="tabs">
      <button
        class="tab-button"
        class:active={activeTab === "general"}
        onclick={() => (activeTab = "general")}
      >
        General
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "visuals"}
        onclick={() => (activeTab = "visuals")}
      >
        Visuals
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "notifications"}
        onclick={() => (activeTab = "notifications")}
      >
        Notifications
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "thresholds"}
        onclick={() => (activeTab = "thresholds")}
      >
        Thresholds
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "capacity"}
        onclick={() => (activeTab = "capacity")}
      >
        Unused Capacity
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "web"}
        onclick={() => (activeTab = "web")}
      >
        Web
      </button>
    </div>

    <div class="tab-content">
      {#if activeTab === "general"}
        <section class="card">
          <h2>Polling Interval</h2>
          <div class="field-row">
            <label for="poll-interval">Check usage every</label>
            <div class="input-with-suffix">
              <input
                id="poll-interval"
                type="number"
                min="1"
                step="1"
                bind:value={pollIntervalMinutes}
                onchange={savePollInterval}
              />
              <span class="suffix">minutes</span>
            </div>
          </div>
          <p class="hint">
            Each check fetches your usage from Claude's servers. Please be considerate —
            polling very frequently puts unnecessary load on Claude's systems and won't make
            readings more accurate. Every 5–15 minutes is plenty.
          </p>
          {#if pollIntervalMinutes < MIN_RECOMMENDED_INTERVAL}
            <p class="notice notice-warn" role="alert">
              ⚠ Checking more often than every {MIN_RECOMMENDED_INTERVAL} minutes is discouraged —
              it hammers Claude's servers without any benefit. Consider a longer interval.
            </p>
          {/if}
        </section>

        <section class="card">
          <h2>Timezone</h2>
          <div class="field-row">
            <label for="timezone">Display times in</label>
            <input
              id="timezone"
              type="text"
              placeholder="local or e.g. America/New_York"
              bind:value={timezone}
              onchange={saveTimezone}
            />
          </div>
          <p class="hint">Use "local" or a timezone like America/New_York, Europe/London, etc.</p>
        </section>

        <section class="card">
          <h2>Launch Behavior</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Open at login</span>
              <p class="hint">Automatically launch Claudar when you sign in.</p>
            </div>
            <Switch
              checked={autostartEnabled}
              label="Toggle open at login"
              onToggle={toggleAutostart}
            />
          </div>
          {#if autostartError}
            <p class="field-error">{autostartError}</p>
          {/if}

          <div class="toggle-row">
            <div>
              <span class="toggle-label" class:label-disabled={!autostartEnabled}>Start minimized</span>
              <p class="hint">
                When opened at login, start hidden in the background instead of showing the
                window. {autostartEnabled ? "Reopen it any time from the menu bar icon or the dock." : "Enable “Open at login” above to use this."}
              </p>
            </div>
            <Switch
              checked={startMinimized}
              disabled={!autostartEnabled}
              label="Toggle start minimized at login"
              onToggle={toggleStartMinimized}
            />
          </div>
        </section>

        <section class="card">
          <h2>Menu Bar</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Show menu bar icon</span>
              <p class="hint">
                Add a Claudar icon to the menu bar. Click it for an at-a-glance
                dropdown of your 5-hour and 7-day usage and reset times.
              </p>
            </div>
            <Switch
              checked={trayIconEnabled}
              label="Toggle menu bar icon"
              onToggle={toggleTrayIcon}
            />
          </div>
        </section>

        <section class="card">
          <h2>Usage History</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Record usage history</span>
              <p class="hint">Save usage snapshots to analyze trends and patterns.</p>
            </div>
            <Switch
              checked={historyEnabled}
              label="Toggle history recording"
              onToggle={toggleHistoryEnabled}
            />
          </div>

          <div class="field-row">
            <label for="history-days">Keep history for</label>
            <div class="input-with-suffix">
              <input
                id="history-days"
                type="number"
                min="1"
                max="365"
                bind:value={historyDays}
                onchange={saveHistoryDays}
                disabled={!historyEnabled}
              />
              <span class="suffix">days</span>
            </div>
          </div>
          <div class="history-presets">
            {#each [7, 14, 30, 90] as days}
              <button
                class="preset-chip"
                class:selected={historyDays === days}
                disabled={!historyEnabled}
                onclick={() => { historyDays = days; saveHistoryDays(); }}
              >
                {days === 7 ? "1 week" : days === 14 ? "2 weeks" : days === 30 ? "1 month" : "3 months"}
              </button>
            {/each}
          </div>
          <p class="hint storage-hint">
            ≈ {recordsFromDays(historyDays).toLocaleString()} records · ~{formatSize(recordsFromDays(historyDays) * BYTES_PER_RECORD)} · older records deleted automatically
          </p>
        </section>

      {/if}

      {#if activeTab === "visuals"}
        <section class="card">
          <h2>Theme</h2>
          <p class="hint">Choose how Claudar looks. System follows your device appearance.</p>
          <div class="theme-options" role="radiogroup" aria-label="Colour theme">
            {#each THEME_OPTIONS as option}
              <button
                type="button"
                class="theme-option"
                class:selected={theme.preference === option.preference}
                role="radio"
                aria-checked={theme.preference === option.preference}
                onclick={() => setTheme(option.preference)}
              >
                {option.label}
              </button>
            {/each}
          </div>
        </section>

        <section class="card">
          <h2>Accessibility</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Reduce motion</span>
              <p class="hint">Stop interface animations and transitions. Your system motion preference is always respected too.</p>
            </div>
            <Switch
              checked={visuals.reduceMotion}
              label="Toggle reduced motion"
              onToggle={toggleReduceMotion}
            />
          </div>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">High contrast</span>
              <p class="hint">Strengthen text, borders, and status treatment for easier reading.</p>
            </div>
            <Switch
              checked={visuals.highContrast}
              label="Toggle high contrast"
              onToggle={toggleHighContrast}
            />
          </div>
          <div class="status-palette-row">
            <div>
              <span class="toggle-label">Status colours</span>
              <p class="hint">Choose a status palette that is easier for you to distinguish.</p>
            </div>
            <div class="palette-options" role="radiogroup" aria-label="Status colour palette">
              {#each (["standard", "colorblind"] as StatusPalette[]) as palette}
                <button
                  type="button"
                  class="palette-option"
                  class:selected={visuals.statusPalette === palette}
                  role="radio"
                  aria-checked={visuals.statusPalette === palette}
                  onclick={() => setStatusPalette(palette)}
                >
                  {palette === "standard" ? "Standard" : "Colour-blind friendly"}
                </button>
              {/each}
            </div>
          </div>
        </section>

        <section class="card">
          <h2>Dashboard</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Show pace delta</span>
              <p class="hint">
                On each usage bar, show how far usage is running ahead or behind the
                elapsed time in the window (e.g. "+8%" or "−8%").
              </p>
            </div>
            <Switch
              checked={showPaceDelta}
              label="Toggle pace delta"
              onToggle={toggleShowPaceDelta}
            />
          </div>
        </section>
      {/if}

      {#if activeTab === "thresholds"}
        <section class="card">
          <h2>5-Hour Limit Thresholds</h2>
          <ul class="threshold-chips">
            {#each fiveHourThresholds as t (t)}
              <li class="chip">
                <span>{t}%</span>
                <button
                  type="button"
                  class="chip-remove"
                  aria-label={`Remove ${t}% threshold`}
                  onclick={() => removeThreshold("five_hour", t)}
                  disabled={fiveHourThresholds.length <= 1}
                >×</button>
              </li>
            {/each}
          </ul>
          <div class="field-row">
            <label for="five-hour-new-threshold">Add threshold</label>
            <div class="input-with-suffix">
              <input
                id="five-hour-new-threshold"
                type="number"
                min="0"
                max="100"
                step="1"
                placeholder="e.g. 85"
                bind:value={fiveHourNewThreshold}
                onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); addThreshold("five_hour"); } }}
                disabled={fiveHourThresholds.length >= MAX_THRESHOLDS}
              />
              <span class="suffix">%</span>
              <button
                type="button"
                class="add-threshold-button"
                onclick={() => addThreshold("five_hour")}
                disabled={fiveHourThresholds.length >= MAX_THRESHOLDS}
              >Add</button>
            </div>
          </div>
          <p class="hint">
            You'll be notified each time usage crosses one of these percentages. Keep 1–5 thresholds;
            they're kept sorted automatically.
            {#if fiveHourThresholds.length >= MAX_THRESHOLDS}Maximum of {MAX_THRESHOLDS} reached — remove one to add another.{/if}
          </p>
        </section>

        <section class="card">
          <h2>7-Day Limit Thresholds</h2>
          <ul class="threshold-chips">
            {#each sevenDayThresholds as t (t)}
              <li class="chip">
                <span>{t}%</span>
                <button
                  type="button"
                  class="chip-remove"
                  aria-label={`Remove ${t}% threshold`}
                  onclick={() => removeThreshold("seven_day", t)}
                  disabled={sevenDayThresholds.length <= 1}
                >×</button>
              </li>
            {/each}
          </ul>
          <div class="field-row">
            <label for="seven-day-new-threshold">Add threshold</label>
            <div class="input-with-suffix">
              <input
                id="seven-day-new-threshold"
                type="number"
                min="0"
                max="100"
                step="1"
                placeholder="e.g. 85"
                bind:value={sevenDayNewThreshold}
                onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); addThreshold("seven_day"); } }}
                disabled={sevenDayThresholds.length >= MAX_THRESHOLDS}
              />
              <span class="suffix">%</span>
              <button
                type="button"
                class="add-threshold-button"
                onclick={() => addThreshold("seven_day")}
                disabled={sevenDayThresholds.length >= MAX_THRESHOLDS}
              >Add</button>
            </div>
          </div>
          <p class="hint">
            You'll be notified each time usage crosses one of these percentages. Keep 1–5 thresholds;
            they're kept sorted automatically.
            {#if sevenDayThresholds.length >= MAX_THRESHOLDS}Maximum of {MAX_THRESHOLDS} reached — remove one to add another.{/if}
          </p>
        </section>
      {/if}

      {#if activeTab === "notifications"}
        <section class="card">
          <h2>Test Notifications</h2>
          <p class="hint">Send a test notification using your current settings below — useful while you tune sound and alert preferences.</p>
          <button class="test-button" onclick={testNotification} disabled={testingNotification}>
            {testingNotification ? "Sending…" : "Send Test Notification"}
          </button>
        </section>

        <section class="card">
          <h2>Sound &amp; Display</h2>
          <p class="hint">Applies to every notification Claudar sends, regardless of which alert types below are enabled.</p>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Sound</span>
              <p class="hint">Play a sound when notifications arrive.</p>
            </div>
            <Switch
              checked={soundEnabled}
              label="Toggle notification sound"
              onToggle={toggleSound}
            />
          </div>

          <div class="field-row">
            <label for="sound-name">Sound</label>
            <select
              id="sound-name"
              bind:value={soundName}
              onchange={saveSoundName}
              disabled={!soundEnabled}
            >
              {#each SOUND_OPTIONS as name}
                <option value={name}>{name}</option>
              {/each}
            </select>
          </div>
          <p class="hint">Choose which system sound plays. Send a test notification above to preview it.</p>

          <div class="toggle-row">
            <div>
              <span class="toggle-label">Persistent notifications</span>
              <p class="hint">Keep notifications on screen until you dismiss them.</p>
            </div>
            <Switch
              checked={persistentEnabled}
              label="Toggle persistent notifications"
              onToggle={togglePersistent}
            />
          </div>
        </section>

        <section class="card">
          <h2>Alert Types</h2>
          <p class="hint">Turn individual kinds of alerts on or off. Each uses the sound and display settings above.</p>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Threshold crossing alerts</span>
              <p class="hint">Notify when usage crosses a threshold percentage.</p>
            </div>
            <Switch
              checked={notifyThresholdCrossings}
              label="Toggle threshold crossing alerts"
              onToggle={toggleNotifyThresholdCrossings}
            />
          </div>

          <div class="toggle-row">
            <div>
              <span class="toggle-label">Predicted overage alerts</span>
              <p class="hint">Warn if your current usage suggests you'll exceed a limit.</p>
            </div>
            <Switch
              checked={notifyPredictedOverage}
              label="Toggle predicted overage alerts"
              onToggle={toggleNotifyPredictedOverage}
            />
          </div>
        </section>

        <section class="card">
          <h2>Reset Notifications</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Notify when limits reset</span>
              <p class="hint">Send an alert the moment your 5-hour or 7-day limit resets.</p>
            </div>
            <Switch
              checked={notifyResets}
              label="Toggle reset notifications"
              onToggle={toggleNotifyResets}
            />
          </div>

          <h3>Pre-Reset Reminders</h3>
          <p class="hint">
            Get a heads-up shortly before a reset happens, so you can plan token-intensive work.
            These reminders are part of reset notifications above — turn that off and these are
            disabled too.
          </p>
          <div class="field-row">
            <label for="before-five-hour" class:label-disabled={!notifyResets}>Alert X minutes before 5-hour resets</label>
            <div class="input-with-suffix">
              <input
                id="before-five-hour"
                type="number"
                min="1"
                placeholder="Leave empty to disable"
                bind:value={minutesBeforeFiveHourReset}
                onchange={saveMinutesBeforeFiveHourReset}
                disabled={!notifyResets}
              />
              <span class="suffix">min</span>
            </div>
          </div>
          <div class="field-row">
            <label for="before-seven-day" class:label-disabled={!notifyResets}>Alert X minutes before 7-day resets</label>
            <div class="input-with-suffix">
              <input
                id="before-seven-day"
                type="number"
                min="1"
                placeholder="Leave empty to disable"
                bind:value={minutesBeforeSevenDayReset}
                onchange={saveMinutesBeforeSevenDayReset}
                disabled={!notifyResets}
              />
              <span class="suffix">min</span>
            </div>
          </div>
        </section>
      {/if}

      {#if activeTab === "capacity"}
        <section class="card">
          <h2>5-Hour Window</h2>
          <p class="hint">
            Send a one-time alert when the 5-hour reset is approaching <strong>and</strong> you
            still have a lot of tokens left — a nudge to use your remaining capacity before the
            window resets.
          </p>
          <div class="field-row">
            <label for="cap-5h-minutes">Reset is less than</label>
            <div class="input-with-suffix">
              <input
                id="cap-5h-minutes"
                type="number"
                min="1"
                placeholder="e.g. 30"
                bind:value={capacityWarningFiveHourMinutes}
                onchange={saveCapacityWarningFiveHour}
              />
              <span class="suffix">minutes away</span>
            </div>
          </div>
          <div class="field-row">
            <label for="cap-5h-percent">And at least</label>
            <div class="input-with-suffix">
              <input
                id="cap-5h-percent"
                type="number"
                min="0"
                max="100"
                placeholder="e.g. 40"
                bind:value={capacityWarningFiveHourPercent}
                onchange={saveCapacityWarningFiveHour}
              />
              <span class="suffix">% of tokens are still unused</span>
            </div>
          </div>
          <p class="hint">Leave either field blank to disable this alert.</p>
        </section>

        <section class="card">
          <h2>7-Day Window</h2>
          <p class="hint">
            Send a one-time alert when the 7-day reset is approaching <strong>and</strong> you
            still have a lot of tokens left — a nudge to use your remaining capacity before the
            window resets.
          </p>
          <div class="field-row">
            <label for="cap-7d-minutes">Reset is less than</label>
            <div class="input-with-suffix">
              <input
                id="cap-7d-minutes"
                type="number"
                min="1"
                placeholder="e.g. 60"
                bind:value={capacityWarningSevenDayMinutes}
                onchange={saveCapacityWarningSevenDay}
              />
              <span class="suffix">minutes away</span>
            </div>
          </div>
          <div class="field-row">
            <label for="cap-7d-percent">And at least</label>
            <div class="input-with-suffix">
              <input
                id="cap-7d-percent"
                type="number"
                min="0"
                max="100"
                placeholder="e.g. 40"
                bind:value={capacityWarningSevenDayPercent}
                onchange={saveCapacityWarningSevenDay}
              />
              <span class="suffix">% of tokens are still unused</span>
            </div>
          </div>
          <p class="hint">Leave either field blank to disable this alert.</p>
        </section>
      {/if}

      {#if activeTab === "web"}
        <section class="card">
          <h2>Web Dashboard</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Serve a read-only dashboard</span>
              <p class="hint">
                Check usage from another device (e.g. your phone over Tailscale) at
                <code>http://{webBind}:{webPort}</code>. No login required, so only bind
                this to a network you trust.
              </p>
            </div>
            <Switch
              checked={webEnabled}
              label="Toggle web dashboard"
              onToggle={toggleWebEnabled}
            />
          </div>

          <div class="field-row">
            <label for="web-bind" class:label-disabled={!webEnabled}>Bind address</label>
            <input
              id="web-bind"
              type="text"
              placeholder="127.0.0.1"
              bind:value={webBind}
              onchange={saveWebBind}
              disabled={!webEnabled}
            />
          </div>
          <p class="hint">
            Defaults to loopback-only (127.0.0.1). Set this to your Tailscale IP
            (run <code>tailscale ip -4</code> on this machine) to reach it from other
            devices on your tailnet — avoid 0.0.0.0, which exposes it on every network
            this machine is connected to.
          </p>

          <div class="field-row">
            <label for="web-port" class:label-disabled={!webEnabled}>Port</label>
            <input
              id="web-port"
              type="number"
              min="1"
              max="65535"
              bind:value={webPort}
              onchange={saveWebPort}
              disabled={!webEnabled}
            />
          </div>
        </section>

        <section class="card">
          <h2>Agent API</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label" class:label-disabled={!webEnabled}>Expose a minimal JSON endpoint for agents</span>
              <p class="hint">
                Adds <code>GET /api/agent</code> alongside the dashboard — a compact,
                machine-readable snapshot per instance (usage %, pace vs. time elapsed,
                reset countdown, predicted peak) meant for other tools or agents to poll
                cheaply, e.g. to decide when to throttle work. No history, no HTML.
                {#if !webEnabled}Enable the web dashboard above to use this.{/if}
              </p>
            </div>
            <Switch
              checked={agentApiEnabled}
              disabled={!webEnabled}
              label="Toggle agent API"
              onToggle={toggleAgentApi}
            />
          </div>
          {#if agentApiEnabled && webEnabled}
            <p class="hint">
              Available at <code>http://{webBind}:{webPort}/api/agent</code>
            </p>
          {/if}
        </section>
      {/if}


    </div>
  {/if}
</div>

{#if toast}
  <div class="toast" class:error={toast.kind === "error"} role="status">
    {toast.message}
  </div>
{/if}

<style>
  .page {
    padding: 2rem;
    max-width: 800px;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
    color: hsl(var(--foreground));
  }

  h2 {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 0.75rem;
    color: hsl(var(--foreground));
  }

  h3 {
    font-size: 0.875rem;
    font-weight: 600;
    margin: 1rem 0 0.25rem;
    padding-top: 0.75rem;
    color: hsl(var(--foreground));
  }

  .subtitle {
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
    margin: 0 0 1.5rem;
  }

  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-bottom: 1.5rem;
  }

  .tab-button {
    padding: 0.65rem 1rem;
    background: none;
    border: none;
    border-radius: var(--radius);
    font-size: 0.875rem;
    color: hsl(var(--muted-foreground));
    cursor: pointer;
    transition: all 0.15s;
    font-weight: 500;
  }

  .tab-button:hover {
    color: hsl(var(--foreground));
  }

  .tab-button.active {
    color: hsl(var(--foreground));
    background: hsl(var(--muted));
  }

  .tab-content {
    animation: fadeIn 0.15s;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .hint {
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
    margin: 0.15rem 0 0.75rem;
  }

  .card {
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    padding: 1.25rem;
    margin-bottom: 1rem;
  }

  .theme-options {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .theme-option {
    min-width: 5.5rem;
    padding: 0.55rem 0.8rem;
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    background: hsl(var(--background));
    color: hsl(var(--foreground));
    font: inherit;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .theme-option:hover {
    background: hsl(var(--muted));
  }

  .theme-option.selected {
    border-color: hsl(var(--foreground));
    background: hsl(var(--foreground));
    color: hsl(var(--background));
    font-weight: 600;
  }

  .theme-option:focus-visible,
  .palette-option:focus-visible {
    outline: 2px solid hsl(var(--ring));
    outline-offset: 2px;
  }

  .status-palette-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.8rem 0 0;
  }

  .status-palette-row .hint {
    margin-bottom: 0;
    max-width: 360px;
  }

  .palette-options {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .palette-option {
    padding: 0.45rem 0.65rem;
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    background: hsl(var(--background));
    color: hsl(var(--foreground));
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .palette-option:hover {
    background: hsl(var(--muted));
  }

  .palette-option.selected {
    border-color: hsl(var(--foreground));
    background: hsl(var(--foreground));
    color: hsl(var(--background));
    font-weight: 600;
  }

  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.6rem 0;
  }

  .field-row label {
    font-size: 0.875rem;
    color: hsl(var(--foreground));
    flex-shrink: 0;
  }

  .field-row label.label-disabled {
    color: hsl(var(--muted-foreground));
  }

  .field-row input,
  .field-row select {
    padding: 0.4rem 0.6rem;
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-family: inherit;
  }

  .field-row input:disabled,
  .field-row select:disabled {
    background-color: hsl(var(--muted));
    color: hsl(var(--muted-foreground));
    cursor: not-allowed;
  }

  .threshold-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    list-style: none;
    margin: 0 0 1rem;
    padding: 0;
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.3rem 0.3rem 0.3rem 0.75rem;
    border-radius: 999px;
    background-color: hsl(var(--muted));
    color: hsl(var(--foreground));
    font-size: 0.825rem;
    font-variant-numeric: tabular-nums;
  }

  .chip-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.25rem;
    height: 1.25rem;
    border: none;
    border-radius: 50%;
    background: none;
    color: hsl(var(--muted-foreground));
    font-size: 1rem;
    line-height: 1;
    cursor: pointer;
    transition: all 0.12s;
  }

  .chip-remove:hover:not(:disabled) {
    background-color: hsl(var(--danger-bg));
    color: hsl(var(--danger-strong));
  }

  .chip-remove:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .add-threshold-button {
    padding: 0.4rem 0.9rem;
    background-color: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    border: none;
    border-radius: var(--radius);
    font-size: 0.825rem;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .add-threshold-button:hover:not(:disabled) {
    background-color: hsl(var(--primary-hover));
  }

  .add-threshold-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .input-with-suffix {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .input-with-suffix input {
    width: 6rem;
    text-align: right;
  }

  .suffix {
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
    white-space: nowrap;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.8rem 0;
  }

  .toggle-label {
    font-size: 0.875rem;
    font-weight: 600;
    color: hsl(var(--foreground));
  }

  .toggle-label.label-disabled {
    color: hsl(var(--muted-foreground));
  }

  .toggle-row .hint {
    margin: 0.2rem 0 0;
    max-width: 360px;
  }

  .notice {
    font-size: 0.8rem;
    line-height: 1.4;
    margin: 0.5rem 0 0;
    padding: 0.55rem 0.75rem;
    border-radius: var(--radius);
  }

  .notice-warn {
    background-color: hsl(var(--warning-bg));
    color: hsl(var(--warning-strong));
    border: 1px solid hsl(var(--warning-border));
  }

  .test-button {
    padding: 0.65rem 1.25rem;
    background-color: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    border: none;
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .test-button:hover:not(:disabled) {
    background-color: hsl(var(--primary-hover));
  }

  .test-button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .field-error {
    margin: 0.5rem 0 0;
    font-size: 0.8rem;
    color: hsl(var(--danger-strong));
  }

  .error-banner {
    padding: 0.75rem 1rem;
    background-color: hsl(var(--danger-bg));
    color: hsl(var(--danger-strong));
    border-radius: var(--radius);
    font-size: 0.875rem;
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
    to {
      transform: rotate(360deg);
    }
  }

  .toast {
    position: fixed;
    bottom: 1.5rem;
    right: 1.5rem;
    padding: 0.65rem 1.1rem;
    background-color: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    border-radius: var(--radius);
    font-size: 0.85rem;
    box-shadow: 0 8px 30px hsl(222.2 84% 4.9% / 0.3);
    z-index: 100;
  }

  .toast.error {
    background-color: hsl(var(--danger));
  }

  .history-presets {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    margin: 0.25rem 0 0.5rem;
  }

  .preset-chip {
    padding: 0.25rem 0.65rem;
    border: 1px solid hsl(var(--border));
    border-radius: 999px;
    background: none;
    font-size: 0.775rem;
    color: hsl(var(--muted-foreground));
    cursor: pointer;
    transition: all 0.12s;
  }

  .preset-chip:hover:not(:disabled) {
    border-color: hsl(var(--foreground));
    color: hsl(var(--foreground));
  }

  .preset-chip.selected {
    background-color: hsl(var(--primary));
    border-color: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
  }

  .preset-chip:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .storage-hint {
    font-variant-numeric: tabular-nums;
  }
</style>
