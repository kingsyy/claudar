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
      [key: string]: unknown;
    };
    instances: { name: string }[];
    history: { enabled: boolean; max_records: number };
  };

  let config = $state<Config | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  // Form-bound values (kept in sync with `config` on load).
  let pollIntervalMinutes = $state(15);
  let fiveHourThreshold = $state(80);
  let sevenDayThreshold = $state(80);
  let notificationsEnabled = $state(true);

  let autostartEnabled = $state(false);
  let autostartError = $state<string | null>(null);

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
      fiveHourThreshold =
        config.thresholds.five_hour[config.thresholds.five_hour.length - 1] ?? 80;
      sevenDayThreshold =
        config.thresholds.seven_day[config.thresholds.seven_day.length - 1] ?? 80;
      notificationsEnabled =
        config.notifications.notify_threshold_crossings ||
        config.notifications.notify_predicted_overage ||
        config.notifications.notify_resets;
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
    // `config` is a Svelte 5 `$state` proxy — structuredClone can't handle it,
    // so round-trip through JSON to get a plain, mutable copy.
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

  function saveFiveHourThreshold() {
    const pct = Math.min(100, Math.max(1, Math.round(fiveHourThreshold)));
    fiveHourThreshold = pct;
    persist((c) => {
      c.thresholds.five_hour = [...c.thresholds.five_hour.slice(0, -1), pct];
    }, "5-hour warning threshold updated");
  }

  function saveSevenDayThreshold() {
    const pct = Math.min(100, Math.max(1, Math.round(sevenDayThreshold)));
    sevenDayThreshold = pct;
    persist((c) => {
      c.thresholds.seven_day = [...c.thresholds.seven_day.slice(0, -1), pct];
    }, "7-day warning threshold updated");
  }

  function toggleNotifications() {
    const next = !notificationsEnabled;
    notificationsEnabled = next;
    persist((c) => {
      c.notifications.notify_threshold_crossings = next;
      c.notifications.notify_predicted_overage = next;
      c.notifications.notify_resets = next;
    }, next ? "Notifications enabled" : "Notifications disabled");
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

  onMount(async () => {
    await Promise.all([loadConfig(), loadAutostart()]);
  });
</script>

<div class="page">
  <h1>Settings</h1>
  <p class="subtitle">Changes are applied immediately — no restart required.</p>

  {#if loadError}
    <div class="error-banner" role="alert">⚠ {loadError}</div>
  {:else if loading}
    <div class="loading">
      <div class="spinner" aria-hidden="true"></div>
      <p>Loading settings…</p>
    </div>
  {:else if config}
    <section class="card">
      <h2>Polling</h2>
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
      <h2>Warning thresholds</h2>
      <p class="hint">You'll be notified when usage crosses these percentages.</p>
      <div class="field-row">
        <label for="five-hour-threshold">5-hour limit warning</label>
        <div class="input-with-suffix">
          <input
            id="five-hour-threshold"
            type="number"
            min="1"
            max="100"
            step="1"
            bind:value={fiveHourThreshold}
            onchange={saveFiveHourThreshold}
          />
          <span class="suffix">%</span>
        </div>
      </div>
      <div class="field-row">
        <label for="seven-day-threshold">7-day limit warning</label>
        <div class="input-with-suffix">
          <input
            id="seven-day-threshold"
            type="number"
            min="1"
            max="100"
            step="1"
            bind:value={sevenDayThreshold}
            onchange={saveSevenDayThreshold}
          />
          <span class="suffix">%</span>
        </div>
      </div>
    </section>

    <section class="card">
      <h2>Notifications</h2>
      <div class="toggle-row">
        <div>
          <span class="toggle-label">Desktop notifications</span>
          <p class="hint">Threshold crossings, predicted overages, and reset reminders.</p>
        </div>
        <button
          class="switch"
          class:on={notificationsEnabled}
          role="switch"
          aria-checked={notificationsEnabled}
          aria-label="Toggle desktop notifications"
          onclick={toggleNotifications}
        >
          <span class="switch-thumb"></span>
        </button>
      </div>
    </section>

    <section class="card">
      <h2>Startup</h2>
      <div class="toggle-row">
        <div>
          <span class="toggle-label">Open at login</span>
          <p class="hint">Launch Claude Notify automatically when you sign in.</p>
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
</div>

{#if toast}
  <div class="toast" class:error={toast.kind === "error"} role="status">
    {toast.message}
  </div>
{/if}

<style>
  .page {
    padding: 2rem;
    max-width: 640px;
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
    padding: 0.4rem 0;
  }

  .field-row label {
    font-size: 0.875rem;
    color: hsl(222.2 84% 4.9%);
  }

  .input-with-suffix {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .input-with-suffix input {
    width: 5rem;
    padding: 0.4rem 0.6rem;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-family: inherit;
    text-align: right;
  }

  .suffix {
    font-size: 0.8rem;
    color: hsl(215.4 16.3% 46.9%);
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
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
