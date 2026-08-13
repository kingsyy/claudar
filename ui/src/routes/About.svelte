<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  // `embedded` renders this screen as a Settings tab panel — no page chrome and no
  // second <h1>, since the Settings heading and the active tab already name it.
  let { embedded = false }: { embedded?: boolean } = $props();

  type AboutInfo = {
    version: string;
    git_hash: string;
    license: string;
    repository: string;
  };

  let info = $state<AboutInfo | null>(null);
  let loadError = $state<string | null>(null);

  onMount(async () => {
    try {
      info = await invoke<AboutInfo>("about_info");
    } catch (e) {
      loadError = String(e);
    }
  });

  async function openRepo() {
    if (!info) return;
    try {
      await invoke("open_url", { url: info.repository });
    } catch (e) {
      loadError = String(e);
    }
  }
</script>

<div class="page" class:embedded>
  {#if !embedded}
    <h1>About</h1>
  {/if}
  <p class="subtitle">Claudar — Claude.ai usage limit monitor.</p>

  {#if loadError}
    <div class="error-banner" role="alert">⚠ {loadError}</div>
  {/if}

  {#if info}
    <section class="card">
      <div class="brand">
        <svg class="logo-mark" viewBox="0 0 100 100" aria-hidden="true">
          <rect width="100" height="100" rx="22" fill="#1A1A1A" />
          <circle cx="50" cy="50" r="30" fill="none" stroke="#D97757" stroke-width="4" opacity="0.35" />
          <circle cx="50" cy="50" r="20" fill="none" stroke="#D97757" stroke-width="4.5" opacity="0.65" />
          <circle cx="50" cy="50" r="8" fill="#D97757" />
        </svg>
        <div>
          <div class="brand-name">Claudar</div>
          <div class="brand-version">v{info.version}</div>
        </div>
      </div>

      <dl class="info-list">
        <div class="info-row">
          <dt>Version</dt>
          <dd>{info.version}</dd>
        </div>
        <div class="info-row">
          <dt>Build</dt>
          <dd class="mono">{info.git_hash}</dd>
        </div>
        <div class="info-row">
          <dt>License</dt>
          <dd>{info.license}</dd>
        </div>
        <div class="info-row">
          <dt>Source</dt>
          <dd>
            <button class="link" onclick={openRepo}>
              {info.repository.replace("https://", "")} ↗
            </button>
          </dd>
        </div>
      </dl>
    </section>
  {:else if !loadError}
    <div class="loading">
      <div class="spinner" aria-hidden="true"></div>
    </div>
  {/if}
</div>

<style>
  .page {
    padding: 2rem;
    max-width: 800px;
  }

  /* Inside a Settings tab the surrounding page already supplies padding and width. */
  .page.embedded {
    padding: 0;
    max-width: none;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
    color: hsl(var(--foreground));
  }

  .subtitle {
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
    margin: 0 0 1.5rem;
  }

  .card {
    background-color: hsl(var(--card));
    border: 1px solid hsl(var(--border));
    border-radius: 0.5rem;
    padding: 1.5rem;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding-bottom: 1.25rem;
    margin-bottom: 1.25rem;
    border-bottom: 1px solid hsl(var(--border));
  }

  .logo-mark {
    width: 2.75rem;
    height: 2.75rem;
    flex-shrink: 0;
    display: block;
  }

  .brand-name {
    font-size: 1.125rem;
    font-weight: 600;
    color: hsl(var(--foreground));
  }

  .brand-version {
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
  }

  .info-list {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .info-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }

  dt {
    font-size: 0.875rem;
    color: hsl(var(--muted-foreground));
  }

  dd {
    margin: 0;
    font-size: 0.875rem;
    color: hsl(var(--foreground));
    text-align: right;
  }

  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.8rem;
  }

  .link {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: hsl(var(--primary, 222 47% 40%));
    cursor: pointer;
    text-decoration: none;
  }

  .link:hover {
    text-decoration: underline;
  }

  .error-banner {
    background-color: hsl(0 84% 60% / 0.1);
    border: 1px solid hsl(0 84% 60% / 0.3);
    color: hsl(0 72% 51%);
    padding: 0.75rem 1rem;
    border-radius: 0.5rem;
    font-size: 0.875rem;
    margin-bottom: 1.5rem;
  }

  .loading {
    display: flex;
    justify-content: center;
    padding: 2rem;
  }

  .spinner {
    width: 1.5rem;
    height: 1.5rem;
    border: 2px solid hsl(var(--border));
    border-top-color: hsl(var(--foreground));
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
