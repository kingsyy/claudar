<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type Config = {
    general: { poll_interval_seconds: number; timezone: string };
    thresholds: { five_hour: number[]; seven_day: number[] };
    notifications: {
      sound: boolean;
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
  };

  let config = $state<Config | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let activeTab = $state<"polling" | "thresholds" | "notifications" | "capacity" | "history" | "startup" | "test">("polling");

  // Form-bound values
  let pollIntervalMinutes = $state(15);
  let timezone = $state("local");
  let fiveHourThresholdStr = $state("50,75,90,100");
  let sevenDayThresholdStr = $state("50,75,90,100");

  let soundEnabled = $state(true);
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
  let historyMaxRecords = $state(2016);

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
      fiveHourThresholdStr = config.thresholds.five_hour.join(",");
      sevenDayThresholdStr = config.thresholds.seven_day.join(",");

      soundEnabled = config.notifications.sound;
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
      historyMaxRecords = config.history.max_records;
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
    persist((c) => {
      c.general.poll_interval_seconds = minutes * 60;
    }, "Polling interval updated");
  }

  function saveTimezone() {
    persist((c) => {
      c.general.timezone = timezone;
    }, "Timezone updated");
  }

  function parseCsvThresholds(csv: string): number[] | null {
    try {
      const parts = csv.split(",").map((s) => {
        const n = parseInt(s.trim(), 10);
        if (isNaN(n) || n < 0 || n > 100) throw new Error(`Invalid percentage: ${s}`);
        return n;
      });
      if (parts.length === 0) throw new Error("At least one threshold required");
      return parts;
    } catch (e) {
      showToast(String(e), "error");
      return null;
    }
  }

  function saveFiveHourThresholds() {
    const thresholds = parseCsvThresholds(fiveHourThresholdStr);
    if (!thresholds) return;
    persist((c) => {
      c.thresholds.five_hour = thresholds;
    }, "5-hour thresholds updated");
  }

  function saveSevenDayThresholds() {
    const thresholds = parseCsvThresholds(sevenDayThresholdStr);
    if (!thresholds) return;
    persist((c) => {
      c.thresholds.seven_day = thresholds;
    }, "7-day thresholds updated");
  }

  function toggleSound() {
    soundEnabled = !soundEnabled;
    persist((c) => {
      c.notifications.sound = soundEnabled;
    }, soundEnabled ? "Notification sounds enabled" : "Notification sounds disabled");
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

  function saveHistoryMaxRecords() {
    const records = Math.max(1, Math.round(historyMaxRecords));
    historyMaxRecords = records;
    persist((c) => {
      c.history.max_records = records;
    }, "History retention updated");
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
        class:active={activeTab === "polling"}
        onclick={() => (activeTab = "polling")}
      >
        Polling & General
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
        class:active={activeTab === "notifications"}
        onclick={() => (activeTab = "notifications")}
      >
        Notifications
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "capacity"}
        onclick={() => (activeTab = "capacity")}
      >
        Capacity Warnings
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "history"}
        onclick={() => (activeTab = "history")}
      >
        History
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "startup"}
        onclick={() => (activeTab = "startup")}
      >
        Startup
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "test"}
        onclick={() => (activeTab = "test")}
      >
        Test
      </button>
    </div>

    <div class="tab-content">
      {#if activeTab === "polling"}
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
      {/if}

      {#if activeTab === "thresholds"}
        <section class="card">
          <h2>5-Hour Limit Thresholds</h2>
          <div class="field-row">
            <label for="five-hour-thresholds">Alert at (comma-separated %)</label>
            <input
              id="five-hour-thresholds"
              type="text"
              placeholder="50,70,90,100"
              bind:value={fiveHourThresholdStr}
              onchange={saveFiveHourThresholds}
              class="threshold-input"
            />
          </div>
          <p class="hint">
            You'll be notified each time usage crosses one of these percentages. Example: 50,70,90,100
          </p>
        </section>

        <section class="card">
          <h2>7-Day Limit Thresholds</h2>
          <div class="field-row">
            <label for="seven-day-thresholds">Alert at (comma-separated %)</label>
            <input
              id="seven-day-thresholds"
              type="text"
              placeholder="50,70,90,100"
              bind:value={sevenDayThresholdStr}
              onchange={saveSevenDayThresholds}
              class="threshold-input"
            />
          </div>
          <p class="hint">
            You'll be notified each time usage crosses one of these percentages. Example: 50,70,90,100
          </p>
        </section>
      {/if}

      {#if activeTab === "notifications"}
        <section class="card">
          <h2>Notification Behavior</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Sound</span>
              <p class="hint">Play a sound when notifications arrive.</p>
            </div>
            <button
              class="switch"
              class:on={soundEnabled}
              role="switch"
              aria-checked={soundEnabled}
              aria-label="Toggle notification sound"
              onclick={toggleSound}
            >
              <span class="switch-thumb"></span>
            </button>
          </div>

          <div class="toggle-row">
            <div>
              <span class="toggle-label">Persistent notifications</span>
              <p class="hint">Keep notifications on screen until you dismiss them.</p>
            </div>
            <button
              class="switch"
              class:on={persistentEnabled}
              role="switch"
              aria-checked={persistentEnabled}
              aria-label="Toggle persistent notifications"
              onclick={togglePersistent}
            >
              <span class="switch-thumb"></span>
            </button>
          </div>
        </section>

        <section class="card">
          <h2>Alert Types</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Threshold crossing alerts</span>
              <p class="hint">Notify when usage crosses a threshold percentage.</p>
            </div>
            <button
              class="switch"
              class:on={notifyThresholdCrossings}
              role="switch"
              aria-checked={notifyThresholdCrossings}
              aria-label="Toggle threshold crossing alerts"
              onclick={toggleNotifyThresholdCrossings}
            >
              <span class="switch-thumb"></span>
            </button>
          </div>

          <div class="toggle-row">
            <div>
              <span class="toggle-label">Predicted overage alerts</span>
              <p class="hint">Warn if your current usage suggests you'll exceed a limit.</p>
            </div>
            <button
              class="switch"
              class:on={notifyPredictedOverage}
              role="switch"
              aria-checked={notifyPredictedOverage}
              aria-label="Toggle predicted overage alerts"
              onclick={toggleNotifyPredictedOverage}
            >
              <span class="switch-thumb"></span>
            </button>
          </div>

          <div class="toggle-row">
            <div>
              <span class="toggle-label">Reset notifications</span>
              <p class="hint">Notify when your usage limits reset.</p>
            </div>
            <button
              class="switch"
              class:on={notifyResets}
              role="switch"
              aria-checked={notifyResets}
              aria-label="Toggle reset notifications"
              onclick={toggleNotifyResets}
            >
              <span class="switch-thumb"></span>
            </button>
          </div>
        </section>

        <section class="card">
          <h2>Pre-Reset Reminders</h2>
          <div class="field-row">
            <label for="before-five-hour">Alert X minutes before 5-hour resets</label>
            <div class="input-with-suffix">
              <input
                id="before-five-hour"
                type="number"
                min="1"
                placeholder="Leave empty to disable"
                bind:value={minutesBeforeFiveHourReset}
                onchange={saveMinutesBeforeFiveHourReset}
              />
              <span class="suffix">min</span>
            </div>
          </div>
          <div class="field-row">
            <label for="before-seven-day">Alert X minutes before 7-day resets</label>
            <div class="input-with-suffix">
              <input
                id="before-seven-day"
                type="number"
                min="1"
                placeholder="Leave empty to disable"
                bind:value={minutesBeforeSevenDayReset}
                onchange={saveMinutesBeforeSevenDayReset}
              />
              <span class="suffix">min</span>
            </div>
          </div>
        </section>
      {/if}

      {#if activeTab === "capacity"}
        <section class="card">
          <h2>5-Hour Capacity Warning</h2>
          <p class="hint">Alert if time is running out AND you have limited capacity left.</p>
          <div class="field-row">
            <label for="cap-5h-minutes">When less than</label>
            <div class="input-with-suffix">
              <input
                id="cap-5h-minutes"
                type="number"
                min="1"
                placeholder="Leave empty to disable"
                bind:value={capacityWarningFiveHourMinutes}
                onchange={saveCapacityWarningFiveHour}
              />
              <span class="suffix">minutes remain</span>
            </div>
          </div>
          <div class="field-row">
            <label for="cap-5h-percent">And capacity is at most</label>
            <div class="input-with-suffix">
              <input
                id="cap-5h-percent"
                type="number"
                min="0"
                max="100"
                placeholder="Leave empty to disable"
                bind:value={capacityWarningFiveHourPercent}
                onchange={saveCapacityWarningFiveHour}
              />
              <span class="suffix">%</span>
            </div>
          </div>
        </section>

        <section class="card">
          <h2>7-Day Capacity Warning</h2>
          <p class="hint">Alert if time is running out AND you have limited capacity left.</p>
          <div class="field-row">
            <label for="cap-7d-minutes">When less than</label>
            <div class="input-with-suffix">
              <input
                id="cap-7d-minutes"
                type="number"
                min="1"
                placeholder="Leave empty to disable"
                bind:value={capacityWarningSevenDayMinutes}
                onchange={saveCapacityWarningSevenDay}
              />
              <span class="suffix">minutes remain</span>
            </div>
          </div>
          <div class="field-row">
            <label for="cap-7d-percent">And capacity is at most</label>
            <div class="input-with-suffix">
              <input
                id="cap-7d-percent"
                type="number"
                min="0"
                max="100"
                placeholder="Leave empty to disable"
                bind:value={capacityWarningSevenDayPercent}
                onchange={saveCapacityWarningSevenDay}
              />
              <span class="suffix">%</span>
            </div>
          </div>
        </section>
      {/if}

      {#if activeTab === "history"}
        <section class="card">
          <h2>Usage History</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Record usage history</span>
              <p class="hint">Save usage snapshots to analyze trends and patterns.</p>
            </div>
            <button
              class="switch"
              class:on={historyEnabled}
              role="switch"
              aria-checked={historyEnabled}
              aria-label="Toggle history recording"
              onclick={toggleHistoryEnabled}
            >
              <span class="switch-thumb"></span>
            </button>
          </div>

          <div class="field-row">
            <label for="history-max">Keep last</label>
            <div class="input-with-suffix">
              <input
                id="history-max"
                type="number"
                min="1"
                bind:value={historyMaxRecords}
                onchange={saveHistoryMaxRecords}
                disabled={!historyEnabled}
              />
              <span class="suffix">records</span>
            </div>
          </div>
          <p class="hint">
            Default (2016 records) = ~14 days at 15-minute polling. Older records are automatically
            deleted.
          </p>
        </section>
      {/if}

      {#if activeTab === "startup"}
        <section class="card">
          <h2>Launch Behavior</h2>
          <div class="toggle-row">
            <div>
              <span class="toggle-label">Open at login</span>
              <p class="hint">Automatically launch Claude Notify when you sign in.</p>
            </div>
            <button
              class="switch"
              class:on={autostartEnabled}
              role="switch"
              aria-checked={autostartEnabled}
              aria-label="Toggle open at login"
              onclick={toggleAutostart}
            >
              <span class="switch-thumb"></span>
            </button>
          </div>
          {#if autostartError}
            <p class="field-error">{autostartError}</p>
          {/if}
        </section>
      {/if}

      {#if activeTab === "test"}
        <section class="card">
          <h2>Test Notification</h2>
          <p class="hint">Send a test notification to verify your notification settings work.</p>
          <button class="test-button" onclick={testNotification} disabled={testingNotification}>
            {testingNotification ? "Sending…" : "Send Test Notification"}
          </button>
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
    color: hsl(222.2 84% 4.9%);
  }

  h2 {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 0.75rem;
    color: hsl(222.2 84% 4.9%);
  }

  .subtitle {
    color: hsl(215.4 16.3% 46.9%);
    font-size: 0.875rem;
    margin: 0 0 1.5rem;
  }

  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    border-bottom: 2px solid hsl(214.3 31.8% 91.4%);
    margin-bottom: 1.5rem;
  }

  .tab-button {
    padding: 0.65rem 1rem;
    background: none;
    border: none;
    border-bottom: 3px solid transparent;
    font-size: 0.875rem;
    color: hsl(215.4 16.3% 46.9%);
    cursor: pointer;
    transition: all 0.15s;
    font-weight: 500;
  }

  .tab-button:hover {
    color: hsl(222.2 84% 4.9%);
  }

  .tab-button.active {
    color: hsl(222.2 84% 4.9%);
    border-bottom-color: hsl(222.2 84% 4.9%);
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
    color: hsl(215.4 16.3% 46.9%);
    margin: 0.15rem 0 0.75rem;
  }

  .card {
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    padding: 1.25rem;
    margin-bottom: 1rem;
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
    color: hsl(222.2 84% 4.9%);
    flex-shrink: 0;
  }

  .field-row input {
    padding: 0.4rem 0.6rem;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-family: inherit;
  }

  .field-row input:disabled {
    background-color: hsl(214.3 31.8% 97%);
    color: hsl(215.4 16.3% 46.9%);
    cursor: not-allowed;
  }

  .threshold-input {
    width: 100%;
    max-width: 300px;
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
    color: hsl(215.4 16.3% 46.9%);
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
    color: hsl(222.2 84% 4.9%);
  }

  .toggle-row .hint {
    margin: 0.2rem 0 0;
    max-width: 360px;
  }

  .switch {
    position: relative;
    width: 42px;
    height: 24px;
    border-radius: 999px;
    border: none;
    background-color: hsl(214.3 31.8% 85%);
    cursor: pointer;
    flex-shrink: 0;
    transition: background-color 0.15s;
  }

  .switch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .switch.on {
    background-color: hsl(222.2 84% 4.9%);
  }

  .switch-thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background-color: hsl(0 0% 100%);
    transition: transform 0.15s;
  }

  .switch.on .switch-thumb {
    transform: translateX(18px);
  }

  .test-button {
    padding: 0.65rem 1.25rem;
    background-color: hsl(222.2 84% 4.9%);
    color: hsl(210 40% 98%);
    border: none;
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .test-button:hover:not(:disabled) {
    background-color: hsl(222.2 84% 15%);
  }

  .test-button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .field-error {
    margin: 0.5rem 0 0;
    font-size: 0.8rem;
    color: hsl(0 70% 45%);
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

  .toast {
    position: fixed;
    bottom: 1.5rem;
    right: 1.5rem;
    padding: 0.65rem 1.1rem;
    background-color: hsl(222.2 84% 4.9%);
    color: hsl(210 40% 98%);
    border-radius: var(--radius);
    font-size: 0.85rem;
    box-shadow: 0 8px 30px hsl(222.2 84% 4.9% / 0.3);
    z-index: 100;
  }

  .toast.error {
    background-color: hsl(0 70% 45%);
  }
</style>
