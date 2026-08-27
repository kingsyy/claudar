<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Wizard from "./Wizard.svelte";

  // `embedded` renders this screen as a Settings tab panel: no page chrome of its
  // own, and its heading drops a level to sit under the Settings <h1>.
  let { embedded = false }: { embedded?: boolean } = $props();

  type InstanceInfo = {
    name: string;
    has_session: boolean;
  };

  let instances = $state<InstanceInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // "Add account" flow
  let addingName = $state(false);
  let newProvider = $state<"claude-web" | "openai-web">("claude-web");
  let newName = $state("");
  let addError = $state<string | null>(null);
  let adding = $state(false);
  let wizardInstance = $state<string | null>(null);
  // When set, the wizard runs in re-login mode (auth step only) for an existing account.
  let reloginInstance = $state<string | null>(null);

  // "Remove" flow
  let pendingRemoval = $state<string | null>(null);
  let removing = $state(false);
  let removeError = $state<string | null>(null);

  // Reorder flow — config order is what the dashboard, tray and web view render in.
  let dragIndex = $state<number | null>(null);
  let reorderError = $state<string | null>(null);

  function moveItem(from: number, to: number) {
    if (to < 0 || to >= instances.length || from === to) return;
    const next = [...instances];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    instances = next;
  }

  async function persistOrder() {
    reorderError = null;
    try {
      await invoke("reorder_instances", { order: instances.map((i) => i.name) });
    } catch (e) {
      reorderError = String(e);
      // Fall back to whatever the backend actually has, so the list can't lie.
      await loadInstances();
    }
  }

  function handleDragStart(e: DragEvent, index: number) {
    dragIndex = index;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", instances[index].name);
    }
  }

  function handleDragOver(e: DragEvent, index: number) {
    if (dragIndex === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    if (dragIndex === index) return;
    moveItem(dragIndex, index);
    dragIndex = index;
  }

  async function handleDragEnd() {
    if (dragIndex === null) return;
    dragIndex = null;
    await persistOrder();
  }

  // Keyboard equivalent of the drag: arrows on the focused handle move the row.
  async function handleHandleKeydown(e: KeyboardEvent, index: number) {
    const delta = e.key === "ArrowUp" ? -1 : e.key === "ArrowDown" ? 1 : 0;
    if (delta === 0) return;
    e.preventDefault();
    const to = index + delta;
    if (to < 0 || to >= instances.length) return;
    moveItem(index, to);
    await persistOrder();
    // Keep focus on the handle of the row we just moved.
    requestAnimationFrame(() => {
      const handles = document.querySelectorAll<HTMLElement>(".drag-handle");
      handles[to]?.focus();
    });
  }

  async function loadInstances() {
    loading = true;
    error = null;
    try {
      instances = await invoke<InstanceInfo[]>("get_instances");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function startAddAccount() {
    addingName = true;
    newName = "";
    newProvider = "claude-web";
    addError = null;
  }

  function cancelAddAccount() {
    addingName = false;
    newName = "";
    addError = null;
  }

  async function submitNewAccount() {
    const name = newName.trim();
    if (!name) {
      addError = "Enter a name for the new account";
      return;
    }
    adding = true;
    addError = null;
    try {
      if (newProvider === "openai-web") {
        // Adds the account and opens the ChatGPT login in one step, so there is
        // no Claude-shaped wizard to hand off to.
        await invoke("add_chatgpt_instance", { name });
        addingName = false;
        await loadInstances();
      } else {
        await invoke("add_instance", { name });
        addingName = false;
        wizardInstance = name;
      }
    } catch (e) {
      addError = String(e);
    } finally {
      adding = false;
    }
  }

  async function finishWizard() {
    wizardInstance = null;
    reloginInstance = null;
    await loadInstances();
  }

  function startRelogin(name: string) {
    reloginInstance = name;
  }

  function requestRemoval(name: string) {
    pendingRemoval = name;
    removeError = null;
  }

  function cancelRemoval() {
    pendingRemoval = null;
    removeError = null;
  }

  async function confirmRemoval() {
    if (!pendingRemoval) return;
    removing = true;
    removeError = null;
    try {
      await invoke("remove_instance", { instance: pendingRemoval });
      pendingRemoval = null;
      await loadInstances();
    } catch (e) {
      removeError = String(e);
    } finally {
      removing = false;
    }
  }

  onMount(loadInstances);
</script>

{#if wizardInstance || reloginInstance}
  <!-- Embedded as a Settings tab, the wizard would otherwise render as a 100vh
       block inside the tab panel. Lift it out so the login flow still owns the
       window, exactly as it does from the top-level route. -->
  <div class:wizard-overlay={embedded}>
    {#if wizardInstance}
      <Wizard instance={wizardInstance} onComplete={finishWizard} />
    {:else if reloginInstance}
      <Wizard instance={reloginInstance} relogin onComplete={finishWizard} />
    {/if}
  </div>
{:else}
  <div class="page" class:embedded>
    <div class="header-row">
      <div>
        {#if embedded}
          <h2>Accounts</h2>
        {:else}
          <h1>Accounts</h1>
        {/if}
        <p class="subtitle">Manage the Claude.ai accounts Claudar monitors.</p>
      </div>
      <button class="primary" onclick={startAddAccount} disabled={addingName}>
        + Add account
      </button>
    </div>

    {#if addingName}
      <div class="add-card">
        <fieldset class="provider-choice">
          <legend>Service</legend>
          <label>
            <input type="radio" bind:group={newProvider} value="claude-web" disabled={adding} />
            Claude.ai
          </label>
          <label>
            <input type="radio" bind:group={newProvider} value="openai-web" disabled={adding} />
            ChatGPT
          </label>
        </fieldset>
        <label for="new-account-name">Account name</label>
        <div class="add-row">
          <input
            id="new-account-name"
            type="text"
            placeholder="e.g. work, personal"
            bind:value={newName}
            disabled={adding}
          />
          <button class="primary" onclick={submitNewAccount} disabled={adding}>
            {adding ? "Adding…" : "Continue"}
          </button>
          <button class="ghost" onclick={cancelAddAccount} disabled={adding}>Cancel</button>
        </div>
        {#if addError}
          <p class="field-error">{addError}</p>
        {/if}
      </div>
    {/if}

    {#if error}
      <div class="error-banner" role="alert">⚠ {error}</div>
    {:else if loading}
      <div class="loading">
        <div class="spinner" aria-hidden="true"></div>
        <p>Loading accounts…</p>
      </div>
    {:else if instances.length === 0}
      <div class="empty-state">
        <p>No accounts configured yet.</p>
      </div>
    {:else}
      {#if reorderError}
        <div class="error-banner" role="alert">⚠ {reorderError}</div>
      {/if}
      {#if instances.length > 1}
        <p class="reorder-hint">Drag an account to change the order it appears in on the dashboard.</p>
      {/if}
      <ul class="account-list">
        {#each instances as inst, index (inst.name)}
          <li
            class="account-row"
            class:dragging={dragIndex === index}
            class:reorderable={instances.length > 1}
            draggable={instances.length > 1}
            ondragstart={(e) => handleDragStart(e, index)}
            ondragover={(e) => handleDragOver(e, index)}
            ondragend={handleDragEnd}
            ondrop={(e) => e.preventDefault()}
          >
            {#if instances.length > 1}
              <button
                type="button"
                class="drag-handle"
                aria-label={`Reorder ${inst.name}. Position ${index + 1} of ${instances.length}. Use arrow up and arrow down to move.`}
                onkeydown={(e) => handleHandleKeydown(e, index)}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true" fill="currentColor">
                  <circle cx="6" cy="4" r="1.3" /><circle cx="10" cy="4" r="1.3" />
                  <circle cx="6" cy="8" r="1.3" /><circle cx="10" cy="8" r="1.3" />
                  <circle cx="6" cy="12" r="1.3" /><circle cx="10" cy="12" r="1.3" />
                </svg>
              </button>
            {/if}
            <div class="account-info">
              <span class="account-name">{inst.name}</span>
              <span class="account-status" class:active={inst.has_session}>
                <span class="status-dot" aria-hidden="true"></span>
                {inst.has_session ? "Active" : "No session"}
              </span>
            </div>
            <div class="account-actions">
              <button class="ghost" onclick={() => startRelogin(inst.name)}>
                {inst.has_session ? "Re-login" : "Login"}
              </button>
              <button class="danger" onclick={() => requestRemoval(inst.name)}>Remove</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if pendingRemoval}
    <div class="modal-backdrop" role="presentation" onclick={cancelRemoval}>
      <div
        class="modal"
        role="alertdialog"
        aria-modal="true"
        aria-label="Confirm account removal"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
      >
        <h2>Remove "{pendingRemoval}"?</h2>
        <p>
          This stops monitoring this account and permanently deletes its stored session,
          state, and history. This cannot be undone.
        </p>
        {#if removeError}
          <p class="field-error">{removeError}</p>
        {/if}
        <div class="modal-actions">
          <button class="ghost" onclick={cancelRemoval} disabled={removing}>Cancel</button>
          <button class="danger" onclick={confirmRemoval} disabled={removing}>
            {removing ? "Removing…" : "Remove account"}
          </button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .page {
    padding: 2rem;
    max-width: 720px;
  }

  /* Inside a Settings tab the surrounding page already supplies padding and width. */
  .page.embedded {
    padding: 0;
    max-width: none;
  }

  .page.embedded h2 {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
    color: hsl(var(--foreground));
  }

  .wizard-overlay {
    position: fixed;
    inset: 0;
    z-index: 60;
    overflow-y: auto;
    background-color: hsl(var(--background));
  }

  .header-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
    color: hsl(var(--foreground));
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
    color: hsl(var(--foreground));
  }

  .subtitle {
    color: hsl(var(--muted-foreground));
    font-size: 0.875rem;
    margin: 0;
  }

  button {
    font-family: inherit;
  }

  .primary {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: var(--radius);
    background-color: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
  }

  .primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .ghost {
    padding: 0.5rem 1rem;
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    background: none;
    color: hsl(var(--foreground));
    font-size: 0.85rem;
    cursor: pointer;
  }

  .danger {
    padding: 0.4rem 0.85rem;
    border: 1px solid hsl(var(--danger-border));
    border-radius: var(--radius);
    background: none;
    color: hsl(var(--danger-strong));
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .danger:hover {
    background-color: hsl(var(--danger-bg));
  }

  .danger:disabled,
  .ghost:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .add-card {
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    padding: 1rem;
    margin-bottom: 1.5rem;
    background-color: hsl(var(--muted));
  }

  .add-card label {
    display: block;
    font-size: 0.8rem;
    font-weight: 600;
    color: hsl(var(--foreground));
    margin-bottom: 0.4rem;
  }

  .add-row {
    display: flex;
    gap: 0.5rem;
  }

  .add-row input {
    flex: 1;
    padding: 0.5rem 0.7rem;
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-family: inherit;
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

  .empty-state {
    padding: 3rem 1rem;
    text-align: center;
    color: hsl(var(--muted-foreground));
    font-size: 0.9rem;
  }

  .account-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .account-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.85rem 1rem;
    border: 1px solid hsl(var(--border));
    border-radius: var(--radius);
  }

  .account-info {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin-right: auto;
  }

  .reorder-hint {
    margin: 0 0 0.75rem;
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
  }

  .account-row.reorderable {
    justify-content: flex-start;
    transition: opacity 0.12s, box-shadow 0.12s, border-color 0.12s;
  }

  .account-row.dragging {
    opacity: 0.55;
    border-color: hsl(var(--ring));
    box-shadow: 0 4px 14px hsl(0 0% 0% / 0.12);
  }

  .drag-handle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.6rem;
    height: 1.9rem;
    padding: 0;
    margin-left: -0.35rem;
    border: none;
    border-radius: var(--radius);
    background: none;
    color: hsl(var(--muted-foreground));
    cursor: grab;
    flex-shrink: 0;
  }

  .drag-handle:hover {
    background-color: hsl(var(--muted));
    color: hsl(var(--foreground));
  }

  .drag-handle:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px hsl(var(--ring) / 0.45);
  }

  .account-row.dragging .drag-handle {
    cursor: grabbing;
  }

  .drag-handle svg {
    width: 14px;
    height: 14px;
  }

  @media (prefers-reduced-motion: reduce) {
    .account-row.reorderable {
      transition: none;
    }
  }

  .account-actions {
    display: flex;
    gap: 0.5rem;
  }

  .account-name {
    font-size: 0.95rem;
    font-weight: 600;
    color: hsl(var(--foreground));
  }

  .account-status {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.8rem;
    color: hsl(var(--muted-foreground));
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: hsl(var(--danger));
  }

  .account-status.active .status-dot {
    background-color: hsl(var(--success));
  }

  .account-status.active {
    color: hsl(var(--success-strong));
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background-color: hsl(222.2 84% 4.9% / 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }

  .modal {
    background-color: hsl(var(--card));
    border-radius: var(--radius);
    padding: 1.5rem;
    max-width: 420px;
    width: calc(100% - 2rem);
    box-shadow: 0 12px 40px hsl(222.2 84% 4.9% / 0.25);
  }

  .modal p {
    font-size: 0.875rem;
    color: hsl(var(--muted-foreground));
    line-height: 1.5;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }
</style>
