<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import Dashboard from "./routes/Dashboard.svelte";
  import History from "./routes/History.svelte";
  import Accounts from "./routes/Accounts.svelte";
  import Settings from "./routes/Settings.svelte";
  import About from "./routes/About.svelte";
  import Wizard from "./routes/Wizard.svelte";
  import { readSidebarCollapsed, saveSidebarCollapsed } from "./lib/sidebar";

  type Route = "dashboard" | "history" | "accounts" | "settings" | "about";

  type InstanceInfo = {
    name: string;
    has_session: boolean;
  };

  let currentRoute: Route = $state("dashboard");
  let menuOpen = $state(false);
  let navCollapsed = $state(false);

  // Responsive: persistent sidebar when there's room, hamburger drawer when narrow.
  // Breakpoint fits the 220px sidebar + a comfortable content column.
  const WIDE_BREAKPOINT = 680;
  let winWidth = $state(typeof window !== "undefined" ? window.innerWidth : 400);
  let wide = $derived(winWidth >= WIDE_BREAKPOINT);

  // `null` = still checking; `true` = show wizard; `false` = go straight to the app.
  let showWizard = $state<boolean | null>(null);

  let unlistenAuthComplete: UnlistenFn | undefined;
  let unlistenWebviewFetch: UnlistenFn | undefined;

  interface WebviewFetchRequest {
    id: number;
    url: string;
    cookie: string;
  }

  const navItems: { id: Route; label: string; icon: string }[] = [
    { id: "history", label: "History", icon: "📈" },
    { id: "accounts", label: "Accounts", icon: "👤" },
    { id: "settings", label: "Settings", icon: "⚙" },
  ];

  function navigate(route: Route) {
    currentRoute = route;
    menuOpen = false;
  }

  function toggleNavCollapsed() {
    navCollapsed = !navCollapsed;
    saveSidebarCollapsed(navCollapsed);
  }

  const routeTitles: Record<Route, string> = {
    dashboard: "Dashboard",
    history: "History",
    accounts: "Accounts",
    settings: "Settings",
    about: "About",
  };

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

  function onResize() {
    winWidth = window.innerWidth;
    if (wide) menuOpen = false; // drawer is meaningless once the sidebar is shown
  }

  onMount(async () => {
    await checkFirstRun();
    navCollapsed = readSidebarCollapsed();
    window.addEventListener("resize", onResize);

    unlistenAuthComplete = await listen<{ instance: string }>("auth-complete", () => {
      checkFirstRun();
    });

    // Proxy HTTP requests from the Rust monitor through WKWebView / URLSession so
    // Cloudflare sees a real browser TLS fingerprint instead of reqwest/rustls.
    unlistenWebviewFetch = await listen<WebviewFetchRequest>("webview-fetch-request", async (event) => {
      const { id, url, cookie } = event.payload;
      try {
        const resp = await fetch(url, {
          headers: {
            Cookie: cookie,
            "User-Agent":
              "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 " +
              "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            Accept: "application/json, text/plain, */*",
            "Accept-Language": "en-US,en;q=0.9",
            Referer: "https://claude.ai/settings/usage",
            Origin: "https://claude.ai",
            "sec-ch-ua": '"Google Chrome";v="131", "Chromium";v="131", "Not_A Brand";v="24"',
            "sec-ch-ua-mobile": "?0",
            "sec-ch-ua-platform": '"macOS"',
            "Sec-Fetch-Dest": "empty",
            "Sec-Fetch-Mode": "cors",
            "Sec-Fetch-Site": "same-origin",
          },
        });
        const body = await resp.text();
        await emit("webview-fetch-response", { id, status: resp.status, body });
      } catch (e: unknown) {
        const message = e instanceof Error ? e.message : String(e);
        await emit("webview-fetch-response", { id, error: message });
      }
    });
  });

  onDestroy(() => {
    window.removeEventListener("resize", onResize);
    unlistenAuthComplete?.();
    unlistenWebviewFetch?.();
  });
</script>

{#snippet logo()}
  <svg class="logo-mark" viewBox="0 0 100 100" aria-hidden="true">
    <rect width="100" height="100" rx="22" fill="#1A1A1A" />
    <circle cx="50" cy="50" r="30" fill="none" stroke="#D97757" stroke-width="4" opacity="0.35" />
    <circle cx="50" cy="50" r="20" fill="none" stroke="#D97757" stroke-width="4.5" opacity="0.65" />
    <circle cx="50" cy="50" r="8" fill="#D97757" />
  </svg>
{/snippet}

{#snippet navMenu(collapsed = false)}
  <ul class="nav-list">
    {#each navItems as item}
      <li>
        <button
          class="nav-item"
          class:collapsed
          class:active={currentRoute === item.id}
          title={collapsed ? item.label : undefined}
          aria-label={collapsed ? item.label : undefined}
          onclick={() => navigate(item.id)}
        >
          <span class="nav-icon" aria-hidden="true">{item.icon}</span>
          <span class="nav-label">{item.label}</span>
        </button>
      </li>
    {/each}
  </ul>
  <div class="menu-footer">
    <button
      class="nav-item"
      class:collapsed
      class:active={currentRoute === "about"}
      title={collapsed ? "About" : undefined}
      aria-label={collapsed ? "About" : undefined}
      onclick={() => navigate("about")}
    >
      <span class="nav-icon" aria-hidden="true">ⓘ</span>
      <span class="nav-label">About</span>
    </button>
  </div>
{/snippet}

{#if showWizard === true}
  <Wizard onComplete={finishWizard} />
{:else if showWizard === false}
  <div class="app-shell" class:wide>
    {#if wide}
      <!-- Roomy: persistent sidebar, no hamburger -->
      <nav class="sidebar" class:collapsed={navCollapsed} aria-label="Primary navigation">
        <button class="sidebar-logo" onclick={() => navigate("dashboard")} aria-label="Go to dashboard">
          {@render logo()}
          <span class="logo-text">Claudar</span>
        </button>
        {@render navMenu(navCollapsed)}
        <!-- Sits at the very bottom: it's chrome, not navigation, so it stays out
             of the way of the items people actually click. -->
        <button
          class="collapse-toggle"
          class:collapsed={navCollapsed}
          onclick={toggleNavCollapsed}
          aria-expanded={!navCollapsed}
          aria-label={navCollapsed ? "Expand navigation" : "Collapse navigation"}
          title={navCollapsed ? "Expand navigation" : "Collapse navigation"}
        >
          <span class="collapse-icon" aria-hidden="true">{navCollapsed ? "›" : "‹"}</span>
          <span class="collapse-label">Collapse</span>
        </button>
      </nav>
    {:else}
      <!-- Narrow: top bar + slide-out drawer -->
      <header class="topbar">
        <button
          class="hamburger"
          class:open={menuOpen}
          onclick={() => (menuOpen = !menuOpen)}
          aria-label={menuOpen ? "Close menu" : "Open menu"}
          aria-expanded={menuOpen}
        >
          <span></span><span></span><span></span>
        </button>
        <button class="topbar-brand" onclick={() => navigate("dashboard")} aria-label="Go to dashboard">
          {@render logo()}
          <span class="topbar-title">{routeTitles[currentRoute]}</span>
        </button>
      </header>

      {#if menuOpen}
        <button class="scrim" onclick={() => (menuOpen = false)} aria-label="Close menu"></button>
        <nav class="drawer" aria-label="Primary navigation">
          {@render navMenu()}
        </nav>
      {/if}
    {/if}

    <main class="content" class:no-scroll-x={currentRoute === "dashboard"}>
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
{/if}

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  /* Wide: sidebar sits beside the content instead of stacking. */
  .app-shell.wide {
    flex-direction: row;
  }

  /* ── Persistent sidebar (wide layout) ───────────────────────────── */
  .sidebar {
    width: var(--sidebar-width);
    flex-shrink: 0;
    background-color: hsl(var(--sidebar-bg));
    color: hsl(var(--sidebar-fg));
    border-right: 1px solid hsl(var(--sidebar-border));
    display: flex;
    flex-direction: column;
    transition: width 0.18s ease;
  }

  .sidebar.collapsed {
    --sidebar-width: 64px;
  }

  .sidebar-logo {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-height: 4rem;
    padding: 1rem;
    font-weight: 600;
    background: none;
    border: none;
    color: inherit;
    width: 100%;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .sidebar-logo:hover {
    background-color: rgba(255, 255, 255, 0.06);
  }

  .logo-text {
    font-size: 0.9rem;
    letter-spacing: 0.01em;
    white-space: nowrap;
  }

  .sidebar.collapsed .sidebar-logo {
    justify-content: center;
    padding-inline: 0.75rem;
  }

  .sidebar.collapsed .logo-text {
    display: none;
  }

  /* Bottom rail: full-bleed row on a hairline, so it reads as a footer control
     rather than a nav entry. */
  .collapse-toggle {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    margin-top: auto;
    padding: 0.7rem 1rem;
    border: none;
    border-top: 1px solid rgba(255, 255, 255, 0.09);
    background: none;
    color: hsl(var(--sidebar-muted));
    font: inherit;
    font-size: 0.8rem;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.15s, color 0.15s;
  }

  .collapse-toggle:hover {
    background-color: rgba(255, 255, 255, 0.06);
    color: hsl(var(--sidebar-fg));
  }

  .collapse-icon {
    width: 1.25rem;
    text-align: center;
    font-size: 1.1rem;
    line-height: 1;
  }

  .collapse-label {
    white-space: nowrap;
  }

  .collapse-toggle.collapsed {
    justify-content: center;
    gap: 0;
    padding-inline: 0.5rem;
  }

  .collapse-toggle.collapsed .collapse-label {
    display: none;
  }

  /* ── Top bar with hamburger ─────────────────────────────────────── */
  .topbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: 48px;
    flex-shrink: 0;
    padding: 0 0.5rem;
    background-color: hsl(var(--sidebar-bg));
    color: hsl(var(--sidebar-fg));
    border-bottom: 1px solid hsl(var(--sidebar-border));
    z-index: 30;
  }

  .hamburger {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
    width: 2.25rem;
    height: 2.25rem;
    padding: 0 0.5rem;
    background: none;
    border: none;
    cursor: pointer;
    flex-shrink: 0;
  }

  .hamburger span {
    display: block;
    height: 2px;
    width: 1.15rem;
    background: hsl(var(--sidebar-fg));
    border-radius: 2px;
    transition: transform 0.2s ease, opacity 0.2s ease;
  }

  .hamburger.open span:nth-child(1) {
    transform: translateY(6px) rotate(45deg);
  }
  .hamburger.open span:nth-child(2) {
    opacity: 0;
  }
  .hamburger.open span:nth-child(3) {
    transform: translateY(-6px) rotate(-45deg);
  }

  .topbar-brand {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    padding: 0.25rem;
    min-width: 0;
  }

  .logo-mark {
    width: 1.35rem;
    height: 1.35rem;
    flex-shrink: 0;
    display: block;
  }

  .topbar-title {
    font-size: 0.95rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    white-space: nowrap;
  }

  /* ── Slide-out drawer ───────────────────────────────────────────── */
  .scrim {
    position: fixed;
    inset: 48px 0 0 0;
    background: rgba(0, 0, 0, 0.45);
    border: none;
    cursor: default;
    z-index: 20;
    animation: fade-in 0.15s ease;
  }

  .drawer {
    position: fixed;
    top: 48px;
    left: 0;
    bottom: 0;
    width: 220px;
    max-width: 80vw;
    background-color: hsl(var(--sidebar-bg));
    color: hsl(var(--sidebar-fg));
    border-right: 1px solid hsl(var(--sidebar-border));
    display: flex;
    flex-direction: column;
    z-index: 25;
    box-shadow: 2px 0 12px rgba(0, 0, 0, 0.35);
    animation: slide-in 0.2s ease;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slide-in {
    from { transform: translateX(-100%); }
    to { transform: translateX(0); }
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
    gap: 0.75rem;
    width: 100%;
    padding: 0.7rem 1rem;
    background: none;
    border: none;
    color: hsl(var(--sidebar-muted));
    font-size: 0.9rem;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.15s, color 0.15s;
    min-height: 2.6rem;
  }

  .nav-item:hover {
    background-color: rgba(255, 255, 255, 0.06);
    color: hsl(var(--sidebar-fg));
  }

  .nav-item.active {
    background-color: rgba(255, 255, 255, 0.1);
    color: hsl(var(--sidebar-fg));
  }

  .nav-item:focus-visible,
  .sidebar-logo:focus-visible,
  .topbar-brand:focus-visible,
  .hamburger:focus-visible,
  .collapse-toggle:focus-visible {
    outline: 2px solid hsl(var(--sidebar-fg));
    outline-offset: -2px;
  }

  .nav-item.collapsed {
    justify-content: center;
    gap: 0;
    padding-inline: 0.5rem;
  }

  .nav-item.collapsed.active {
    box-shadow: inset 3px 0 0 hsl(var(--sidebar-fg));
  }

  .nav-icon {
    width: 1.25rem;
    text-align: center;
    font-size: 0.85rem;
    flex-shrink: 0;
  }

  .nav-label {
    white-space: nowrap;
  }

  .nav-item.collapsed .nav-label {
    display: none;
  }

  .menu-footer {
    padding-bottom: 0.5rem;
    padding-top: 0.5rem;
  }

  /* ── Content ────────────────────────────────────────────────────── */
  .content {
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    background-color: hsl(var(--background));
  }

  /* Dashboard must fit the narrow window — never scroll sideways. */
  .content.no-scroll-x {
    overflow-x: hidden;
  }
</style>
