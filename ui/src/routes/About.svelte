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

  type UpdateInfo = {
    available: boolean;
    current_version: string;
    version: string | null;
    notes: string | null;
    pub_date: string | null;
    changes_url: string | null;
  };

  let info = $state<AboutInfo | null>(null);
  let loadError = $state<string | null>(null);

  let update = $state<UpdateInfo | null>(null);
  let checking = $state(false);
  let installing = $state(false);
  let updateError = $state<string | null>(null);

  onMount(async () => {
    try {
      info = await invoke<AboutInfo>("about_info");
    } catch (e) {
      loadError = String(e);
    }
    // Check on open rather than behind a button press: the whole point is that
    // someone who never thinks to check still finds out they are behind.
    checkForUpdate();
  });

  async function checkForUpdate() {
    checking = true;
    updateError = null;
    try {
      update = await invoke<UpdateInfo>("check_update");
    } catch (e) {
      updateError = String(e);
    } finally {
      checking = false;
    }
  }

  async function installUpdate() {
    installing = true;
    updateError = null;
    try {
      // On success the app is replaced and relaunched, so this never returns.
      await invoke("install_update");
    } catch (e) {
      updateError = String(e);
      installing = false;
    }
  }

  async function openChanges() {
    if (!update?.changes_url) return;
    try {
      await invoke("open_url", { url: update.changes_url });
    } catch (e) {
      updateError = String(e);
    }
  }

  /** "released 3 days ago" reads better than an RFC 3339 timestamp, and is a
   *  truer staleness signal than a release count -- published version numbers
   *  have gaps, so counting them would be wrong. */
  function releasedAgo(pubDate: string | null): string {
    if (!pubDate) return "";
    const then = new Date(pubDate).getTime();
    if (Number.isNaN(then)) return "";
    const days = Math.floor((Date.now() - then) / 86_400_000);
    if (days < 1) return "released today";
    if (days === 1) return "released yesterday";
    if (days < 30) return `released ${days} days ago`;
    const months = Math.floor(days / 30);
    return months === 1 ? "released a month ago" : `released ${months} months ago`;
  }

  /** Minimal renderer for the changelog subset we publish: `### Heading`,
   *  `- bullet`, and `**bold**`. Avoids pulling in a markdown dependency for
   *  text whose format this repo controls. */
  function renderNotes(notes: string): Array<{ kind: "heading" | "item"; text: string }> {
    const blocks: Array<{ kind: "heading" | "item"; text: string }> = [];
    for (const raw of notes.split("\n")) {
      const line = raw.trim();
      if (line.startsWith("###")) {
        blocks.push({ kind: "heading", text: line.replace(/^#+\s*/, "") });
      } else if (line.startsWith("- ")) {
        blocks.push({ kind: "item", text: line.slice(2) });
      } else if (line && blocks.length && blocks[blocks.length - 1].kind === "item") {
        // Continuation of a wrapped bullet.
        blocks[blocks.length - 1].text += " " + line;
      }
    }
    return blocks.map((b) => ({ ...b, text: b.text.replace(/\*\*/g, "") }));
  }

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
          <div class="brand-version">
            {#if update?.available && update.version}
              v{update.current_version} → v{update.version}
            {:else}
              v{info.version}
            {/if}
          </div>
        </div>
      </div>

      <section class="update" aria-live="polite">
        {#if checking}
          <div class="update-status muted">Checking for updates…</div>
        {:else if updateError}
          <div class="update-status muted">Couldn't check for updates.</div>
          <p class="update-detail">{updateError}</p>
          <button class="btn-secondary" onclick={checkForUpdate}>Try again</button>
        {:else if update?.available && update.version}
          <div class="update-status">
            <strong>Version {update.version} is available</strong>
            {#if releasedAgo(update.pub_date)}
              <span class="muted"> · {releasedAgo(update.pub_date)}</span>
            {/if}
          </div>

          {#if update.notes}
            <div class="notes">
              <div class="notes-title">What's new</div>
              {#each renderNotes(update.notes) as block}
                {#if block.kind === "heading"}
                  <div class="notes-heading">{block.text}</div>
                {:else}
                  <div class="notes-item">{block.text}</div>
                {/if}
              {/each}
            </div>
          {/if}

          <div class="update-actions">
            <button class="btn-primary" onclick={installUpdate} disabled={installing}>
              {installing ? "Downloading…" : "Update and restart"}
            </button>
            {#if update.changes_url}
              <button class="link" onclick={openChanges}>
                All changes since v{update.current_version} ↗
              </button>
            {/if}
          </div>
          {#if installing}
            <p class="update-detail">
              Claudar will close and reopen. Monitoring resumes automatically.
            </p>
          {/if}
        {:else}
          <div class="update-status muted">
            Up to date
            <button class="link check-again" onclick={checkForUpdate}>Check again</button>
          </div>
        {/if}
      </section>

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

  .update {
    padding-bottom: 1.25rem;
    margin-bottom: 1.25rem;
    border-bottom: 1px solid hsl(var(--border));
  }

  .update-status {
    font-size: 0.875rem;
    color: hsl(var(--foreground));
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .update-status.muted,
  .update-status .muted {
    color: hsl(var(--muted-foreground));
  }

  .update-detail {
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
    margin: 0.5rem 0 0;
  }

  .notes {
    margin-top: 0.875rem;
    padding: 0.875rem 1rem;
    background-color: hsl(var(--muted, 210 40% 96%) / 0.5);
    border-radius: 0.375rem;
    max-height: 14rem;
    overflow-y: auto;
  }

  .notes-title {
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: hsl(var(--muted-foreground));
    margin-bottom: 0.5rem;
  }

  .notes-heading {
    font-size: 0.8rem;
    font-weight: 600;
    color: hsl(var(--foreground));
    margin-top: 0.625rem;
  }

  .notes-heading:first-of-type {
    margin-top: 0;
  }

  .notes-item {
    font-size: 0.8rem;
    line-height: 1.5;
    color: hsl(var(--foreground));
    padding-left: 0.875rem;
    text-indent: -0.875rem;
    margin-top: 0.25rem;
  }

  .notes-item::before {
    content: "• ";
    color: hsl(var(--muted-foreground));
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 0.875rem;
    flex-wrap: wrap;
  }

  .btn-primary,
  .btn-secondary {
    font: inherit;
    font-size: 0.8125rem;
    font-weight: 500;
    padding: 0.4375rem 0.875rem;
    border-radius: 0.375rem;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .btn-primary {
    background-color: hsl(var(--primary, 222 47% 40%));
    color: hsl(var(--primary-foreground, 0 0% 100%));
  }

  .btn-primary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .btn-secondary {
    background-color: transparent;
    border-color: hsl(var(--border));
    color: hsl(var(--foreground));
    margin-top: 0.625rem;
  }

  .check-again {
    font-size: 0.8rem;
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
