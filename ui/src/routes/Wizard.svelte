<script lang="ts">
  import { onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";

  type Config = {
    general: { poll_interval_seconds: number; timezone: string };
    thresholds: { five_hour: number[]; seven_day: number[] };
    notifications: Record<string, unknown>;
    instances: { name: string }[];
    history: { enabled: boolean; max_records: number };
  };

  let {
    instance = "default",
    onComplete,
    relogin = false,
  }: { instance?: string; onComplete: () => void; relogin?: boolean } = $props();

  type Step = 1 | 2 | 3 | 4;
  // Re-login only needs the auth step; full onboarding starts at the intro.
  let step = $state<Step>(relogin ? 2 : 1);

  // Step 1 — collapsible sections
  let howItWorksOpen = $state(false);
  let whereIsDataOpen = $state(false);

  // Step 2 — auth
  type AuthState = "idle" | "waiting" | "success" | "error";
  let authState = $state<AuthState>("idle");
  let authError = $state<string | null>(null);
  let whyLoginOpen = $state(false);
  let magicLinkUrl = $state("");
  let unlistenAuthComplete: UnlistenFn | undefined;
  let unlistenAuthError: UnlistenFn | undefined;

  // Step 3 — thresholds
  let fiveHourThreshold = $state(80);
  let sevenDayThreshold = $state(80);
  let savingThresholds = $state(false);
  let thresholdsError = $state<string | null>(null);

  // Step 4 — done
  let pollIntervalMinutes = $state(15);
  let autostartEnabled = $state(false);
  let autostartError = $state<string | null>(null);

  async function loadConfigDefaults() {
    try {
      const config = await invoke<Config>("get_config", { instance });
      if (config.thresholds.five_hour.length > 0) {
        fiveHourThreshold = config.thresholds.five_hour[config.thresholds.five_hour.length - 1];
      }
      if (config.thresholds.seven_day.length > 0) {
        sevenDayThreshold = config.thresholds.seven_day[config.thresholds.seven_day.length - 1];
      }
      pollIntervalMinutes = Math.round(config.general.poll_interval_seconds / 60);
    } catch (e) {
      console.error("get_config failed", e);
    }
  }

  loadConfigDefaults();

  async function startAuth() {
    authState = "waiting";
    authError = null;
    try {
      await invoke("start_auth", { instance });
    } catch (e) {
      authState = "error";
      authError = String(e);
    }
  }

  async function handleMagicLink() {
    if (!magicLinkUrl) return;
    try {
      await invoke("navigate_auth_window", { url: magicLinkUrl });
      magicLinkUrl = "";
    } catch (e) {
      console.error("Magic link error:", e);
    }
  }

  unlistenAuthComplete = undefined;
  (async () => {
    unlistenAuthComplete = await listen<{ instance: string }>("auth-complete", (event) => {
      if (event.payload.instance !== instance) return;
      authState = "success";
      setTimeout(() => {
        if (relogin) {
          onComplete();
        } else {
          step = 3;
        }
      }, 1000);
    });

    unlistenAuthError = await listen<{ instance: string; message: string }>(
      "auth-error",
      (event) => {
        if (event.payload.instance !== instance) return;
        authState = "error";
        authError = event.payload.message;
      },
    );
  })();

  onDestroy(() => {
    unlistenAuthComplete?.();
    unlistenAuthError?.();
  });

  async function saveThresholds() {
    savingThresholds = true;
    thresholdsError = null;
    try {
      const config = await invoke<Config>("get_config", { instance });
      config.thresholds.five_hour = [...config.thresholds.five_hour.slice(0, -1), fiveHourThreshold];
      config.thresholds.seven_day = [...config.thresholds.seven_day.slice(0, -1), sevenDayThreshold];
      await invoke("set_config", { instance, config });
      step = 4;
    } catch (e) {
      thresholdsError = String(e);
    } finally {
      savingThresholds = false;
    }
  }

  async function loadAutostart() {
    try {
      autostartEnabled = await invoke<boolean>("get_autostart");
    } catch (e) {
      console.error("get_autostart failed", e);
    }
  }

  async function toggleAutostart() {
    const next = !autostartEnabled;
    autostartError = null;
    try {
      await invoke("set_autostart", { enabled: next });
      autostartEnabled = next;
    } catch (e) {
      autostartError = String(e);
    }
  }

  $effect(() => {
    if (step === 4) {
      loadAutostart();
    }
  });
</script>

<div class="wizard">
  <div class="wizard-card">
    <div class="step-indicator" aria-hidden="true">
      {#each [1, 2, 3, 4] as n}
        <span class="dot" class:active={step === n} class:done={step > n}></span>
      {/each}
    </div>

    {#if step === 1}
      <section class="step">
        <h1>Never hit a rate limit by surprise</h1>
        <p class="lead">
          Claude Notify watches your usage in the background and sends you a heads-up before you
          run out. It sits in your menu bar, checks every few minutes, and only sees the usage
          percentages — nothing else.
        </p>

        <details bind:open={howItWorksOpen}>
          <summary>How does this work?</summary>
          <p>
            Claude Notify logs into Claude.ai on your behalf using your browser session, then checks
            your usage number every so often. That's it — no messages, no content, just a
            percentage.
          </p>
        </details>

        <details bind:open={whereIsDataOpen}>
          <summary>Where is my data?</summary>
          <p>
            Your session cookies are stored locally on this device, at
            <code>~/.config/claude-notify/sessions/</code>. They're never uploaded or shared.
            Usage numbers are also stored locally at the same path so we can show you a history
            chart. Nothing leaves your device.
          </p>
        </details>

        <button class="primary" onclick={() => (step = 2)}>Get started</button>
      </section>
    {:else if step === 2}
      <section class="step">
        <h1>Log in to Claude.ai</h1>
        <p class="lead">
          Chrome will open to claude.ai/login. Sign in with Google or email — we'll detect
          when you're logged in and close the window automatically.
        </p>

        <button class="link" onclick={() => (whyLoginOpen = true)}>Why do I need to log in?</button>

        {#if whyLoginOpen}
          <div class="sheet" role="dialog" aria-modal="true">
            <div class="sheet-content">
              <h2>Why do I need to log in?</h2>
              <p>
                Claude Notify needs your session cookie to check your usage — the same token your
                browser already uses when you visit claude.ai. It's read-only: we can't send
                messages or change your account. You can revoke access at any time by logging out
                of Claude.ai.
              </p>
              <button class="primary" onclick={() => (whyLoginOpen = false)}>Got it</button>
            </div>
          </div>
        {/if}

        {#if authState === "idle"}
          <button class="primary" onclick={startAuth}>Open login window</button>
        {:else if authState === "waiting"}
          <div class="auth-status">
            <span class="pulse" aria-hidden="true"></span>
            <span>Waiting for login in Chrome…</span>
          </div>
          <details style="margin-top: 1rem; padding: 0.75rem; background: #f5f5f5; border-radius: var(--radius);">
            <summary style="cursor: pointer; font-size: 0.875rem; font-weight: 600;">
              Got a magic link in your email?
            </summary>
            <div style="margin-top: 0.75rem;">
              <p style="font-size: 0.8rem; color: #666; margin: 0 0 0.5rem 0;">
                Paste the link here and we'll open it in Chrome:
              </p>
              <input
                type="url"
                bind:value={magicLinkUrl}
                placeholder="https://claude.ai/magic-link/..."
                style="font-size: 0.8rem; padding: 0.5rem;"
              />
              <button
                class="primary"
                onclick={() => handleMagicLink()}
                disabled={!magicLinkUrl}
                style="font-size: 0.8rem; padding: 0.5rem 1rem; margin-top: 0.5rem;"
              >
                Open link
              </button>
            </div>
          </details>
        {:else if authState === "success"}
          <div class="auth-status success">Connected ✓</div>
        {:else if authState === "error"}
          <div class="auth-status error" role="alert">
            <p>{authError ?? "Something went wrong while logging in."}</p>
            <button class="primary" onclick={startAuth}>Try again</button>
          </div>
        {/if}
      </section>
    {:else if step === 3}
      <section class="step">
        <h1>When should we notify you?</h1>
        <p class="lead">Pick the usage levels where you'd like a heads-up.</p>

        <label class="slider-field">
          <span class="slider-label">
            Notify me when 5-hour usage reaches <strong>{fiveHourThreshold}%</strong>
          </span>
          <input type="range" min="10" max="100" step="5" bind:value={fiveHourThreshold} />
          <span class="hint">
            Your 5-hour usage window resets every 5 hours. Claude stops responding when it hits
            100%.
          </span>
        </label>

        <label class="slider-field">
          <span class="slider-label">
            Notify me when 7-day usage reaches <strong>{sevenDayThreshold}%</strong>
          </span>
          <input type="range" min="10" max="100" step="5" bind:value={sevenDayThreshold} />
          <span class="hint">
            Your 7-day usage window rolls over continuously. Claude stops responding when it hits
            100%.
          </span>
        </label>

        {#if thresholdsError}
          <div class="auth-status error" role="alert">{thresholdsError}</div>
        {/if}

        <button class="primary" onclick={saveThresholds} disabled={savingThresholds}>
          {savingThresholds ? "Saving…" : "Save and continue"}
        </button>
      </section>
    {:else if step === 4}
      <section class="step">
        <h1>You're all set</h1>
        <p class="lead">
          Monitoring your Claude account · checking every {pollIntervalMinutes} minute{pollIntervalMinutes ===
          1
            ? ""
            : "s"}.
        </p>

        <label class="toggle-field">
          <input type="checkbox" checked={autostartEnabled} onchange={toggleAutostart} />
          <span>Start automatically when I log in</span>
        </label>
        {#if autostartError}
          <div class="auth-status error" role="alert">{autostartError}</div>
        {/if}

        <button class="primary" onclick={onComplete}>Open dashboard</button>
      </section>
    {/if}
  </div>
</div>

<style>
  .wizard {
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: hsl(var(--background));
    padding: 2rem;
  }

  .wizard-card {
    width: 100%;
    max-width: 520px;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .step-indicator {
    display: flex;
    gap: 0.5rem;
    justify-content: center;
  }

  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 999px;
    background-color: hsl(var(--border));
  }

  .dot.active {
    background-color: hsl(var(--ring));
  }

  .dot.done {
    background-color: hsl(var(--muted-foreground));
  }

  .step {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0;
    color: hsl(var(--foreground));
  }

  .lead {
    font-size: 0.9rem;
    line-height: 1.5;
    color: hsl(var(--muted-foreground));
    margin: 0;
  }

  details {
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    padding: 0.75rem 1rem;
  }

  details summary {
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 600;
    color: hsl(var(--foreground));
  }

  details p {
    margin: 0.5rem 0 0;
    font-size: 0.85rem;
    line-height: 1.5;
    color: hsl(var(--muted-foreground));
  }

  details code {
    font-size: 0.8em;
    background-color: hsl(var(--secondary));
    padding: 0.1rem 0.3rem;
    border-radius: 0.25rem;
  }

  button.primary {
    align-self: flex-start;
    padding: 0.6rem 1.25rem;
    border: none;
    border-radius: var(--radius);
    background-color: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
  }

  button.primary:hover {
    opacity: 0.9;
  }

  button.primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  button.link {
    align-self: flex-start;
    background: none;
    border: none;
    color: hsl(var(--muted-foreground));
    font-size: 0.8rem;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
  }

  .sheet {
    position: fixed;
    inset: 0;
    background-color: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }

  .sheet-content {
    background-color: hsl(var(--background));
    border-radius: var(--radius);
    padding: 1.5rem;
    max-width: 420px;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .sheet-content h2 {
    margin: 0;
    font-size: 1.1rem;
  }

  .sheet-content p {
    margin: 0;
    font-size: 0.875rem;
    line-height: 1.5;
    color: hsl(var(--muted-foreground));
  }

  .auth-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: hsl(var(--muted-foreground));
  }

  .auth-status.success {
    color: hsl(142 71% 35%);
    font-weight: 600;
  }

  .auth-status.error {
    flex-direction: column;
    align-items: flex-start;
    gap: 0.75rem;
    color: hsl(var(--destructive));
  }

  .pulse {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 999px;
    background-color: hsl(var(--ring));
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.3; transform: scale(0.8); }
    50% { opacity: 1; transform: scale(1.1); }
  }

  .slider-field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .slider-label {
    font-size: 0.875rem;
    color: hsl(var(--foreground));
  }

  .slider-field input[type="range"] {
    width: 100%;
  }

  .hint {
    font-size: 0.75rem;
    color: hsl(var(--muted-foreground));
  }

  .toggle-field {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.875rem;
    color: hsl(var(--foreground));
    cursor: pointer;
  }
</style>
