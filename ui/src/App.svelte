<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import Dashboard from "./routes/Dashboard.svelte";
  import History from "./routes/History.svelte";
  import Accounts from "./routes/Accounts.svelte";
  import Settings from "./routes/Settings.svelte";
  import About from "./routes/About.svelte";
  import Wizard from "./routes/Wizard.svelte";
  import { theme, cycleTheme } from "./lib/theme.svelte";

  const themeMeta: Record<string, { icon: string; label: string }> = {
    light: { icon: "☀", label: "Light" },
    dark: { icon: "☾", label: "Dark" },
    system: { icon: "◐", label: "System" },
  };

  type Route = "dashboard" | "history" | "accounts" | "settings" | "about";

  type InstanceInfo = {
    name: string;
    has_session: boolean;
  };

  let currentRoute: Route = $state("dashboard");

  // `null` = still checking; `true` = show wizard; `false` = go straight to the app.
  let showWizard = $state<boolean | null>(null);

  let unlistenAuthComplete: UnlistenFn | undefined;

  const navItems: { id: Route; label: string; icon: string }[] = [
    { id: "history", label: "History", icon: "📈" },
    { id: "accounts", label: "Accounts", icon: "👤" },
    { id: "settings", label: "Settings", icon: "⚙" },
  ];

  async function testIpc() {
    const result = await invoke<string>("ping");
    console.log("ping →", result);
  }

  testIpc();

  async function checkFirstRun() {
    try {
      const instances = await invoke<InstanceInfo[]>("get_instances");
      showWizard = !instances.some((inst) => inst.has_session);
    } catch (e) {
      console.error("get_instances failed", e);
      showWizard = false;
    }
  }

  function finishWizard() {
    showWizard = false;
  }

  onMount(async () => {
    await checkFirstRun();

    unlistenAuthComplete = await listen<{ instance: string }>("auth-complete", () => {
      checkFirstRun();
    });
  });

  onDestroy(() => {
    unlistenAuthComplete?.();
  });
</script>

{#if showWizard === true}
  <Wizard onComplete={finishWizard} />
{:else if showWizard === false}
  <div class="app-shell">
    <nav class="sidebar">
      <button class="sidebar-logo" onclick={() => (currentRoute = "dashboard")} aria-label="Go to dashboard">
        <svg class="logo-mark" viewBox="0 0 100 100" aria-hidden="true">
          <rect width="100" height="100" rx="22" fill="#1A1A1A" />
          <circle cx="50" cy="50" r="30" fill="none" stroke="#D97757" stroke-width="4" opacity="0.35" />
          <circle cx="50" cy="50" r="20" fill="none" stroke="#D97757" stroke-width="4.5" opacity="0.65" />
          <circle cx="50" cy="50" r="8" fill="#D97757" />
        </svg>
        <span class="logo-text">Claudar</span>
      </button>
      <ul class="nav-list">
        {#each navItems as item}
          <li>
            <button
              class="nav-item"
              class:active={currentRoute === item.id}
              onclick={() => (currentRoute = item.id)}
            >
              <span class="nav-icon" aria-hidden="true">{item.icon}</span>
              <span class="nav-label">{item.label}</span>
            </button>
          </li>
        {/each}
      </ul>
      <div class="sidebar-footer">
        <button
          class="theme-toggle"
          class:active={currentRoute === "about"}
          onclick={() => (currentRoute = "about")}
          title="About Claudar"
          aria-label="About Claudar"
        >
          <span class="nav-icon" aria-hidden="true">ⓘ</span>
          <span class="nav-label">About</span>
        </button>
        <button
          class="theme-toggle"
          onclick={cycleTheme}
          title="Theme: {themeMeta[theme.preference].label} (click to change)"
          aria-label="Switch theme, currently {themeMeta[theme.preference].label}"
        >
          <span class="nav-icon" aria-hidden="true">{themeMeta[theme.preference].icon}</span>
          <span class="nav-label">{themeMeta[theme.preference].label}</span>
        </button>
      </div>
    </nav>

    <div class="main-column">
      <main class="content">
        {#if currentRoute === "dashboard"}
          <Dashboard />
        {:else if currentRoute === "history"}
          <History />
        {:else if currentRoute === "accounts"}
          <Accounts />
        {:else if currentRoute === "settings"}
          <Settings />
        {:else if currentRoute === "about"}
          <About />
        {/if}
      </main>
    </div>
  </div>
{/if}

<style>
  .app-shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .sidebar {
    width: var(--sidebar-width, 220px);
    background-color: hsl(222.2 84% 4.9%);
    color: hsl(210 40% 98%);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }

  .sidebar-logo {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 1.25rem 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    font-weight: 600;
    background: none;
    border-top: none;
    border-left: none;
    border-right: none;
    color: inherit;
    width: 100%;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .sidebar-logo:hover {
    background-color: rgba(255, 255, 255, 0.06);
  }

  .logo-mark {
    width: 1.5rem;
    height: 1.5rem;
    flex-shrink: 0;
    display: block;
  }

  .logo-text {
    font-size: 0.9rem;
    letter-spacing: 0.01em;
  }

  .nav-list {
    list-style: none;
    margin: 0;
    padding: 0.5rem 0;
    flex: 1;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    width: 100%;
    padding: 0.6rem 1rem;
    background: none;
    border: none;
    color: hsl(215.4 16.3% 70%);
    font-size: 0.875rem;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.15s, color 0.15s;
    border-radius: 0;
  }

  .nav-item:hover {
    background-color: rgba(255, 255, 255, 0.06);
    color: hsl(210 40% 98%);
  }

  .nav-item.active {
    background-color: rgba(255, 255, 255, 0.1);
    color: hsl(210 40% 98%);
  }

  .nav-icon {
    width: 1.25rem;
    text-align: center;
    font-size: 0.75rem;
  }

  .sidebar-footer {
    padding-bottom: 0.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    padding-top: 0.5rem;
  }

  .theme-toggle {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    width: 100%;
    padding: 0.6rem 1rem;
    background: none;
    border: none;
    color: hsl(215.4 16.3% 70%);
    font-size: 0.875rem;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.15s, color 0.15s;
  }

  .theme-toggle:hover {
    background-color: rgba(255, 255, 255, 0.06);
    color: hsl(210 40% 98%);
  }

  .theme-toggle.active {
    background-color: rgba(255, 255, 255, 0.1);
    color: hsl(210 40% 98%);
  }

  .theme-toggle .nav-icon {
    font-size: 0.9rem;
  }

  .main-column {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    background-color: hsl(var(--background));
  }
</style>
