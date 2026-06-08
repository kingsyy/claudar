<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import Dashboard from "./routes/Dashboard.svelte";
  import History from "./routes/History.svelte";
  import Accounts from "./routes/Accounts.svelte";
  import Settings from "./routes/Settings.svelte";
  import Wizard from "./routes/Wizard.svelte";

  type Route = "dashboard" | "history" | "accounts" | "settings";

  type InstanceInfo = {
    name: string;
    has_session: boolean;
  };

  let currentRoute: Route = $state("dashboard");

  // `null` = still checking; `true` = show wizard; `false` = go straight to the app.
  let showWizard = $state<boolean | null>(null);

  let unlistenAuthComplete: UnlistenFn | undefined;

  const navItems: { id: Route; label: string; icon: string }[] = [
    { id: "dashboard", label: "Dashboard", icon: "⬤" },
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
      <div class="sidebar-logo">
        <span class="logo-mark">◉</span>
        <span class="logo-text">Claude Notify</span>
      </div>
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
  }

  .logo-mark {
    font-size: 1.25rem;
    color: hsl(210 40% 70%);
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
