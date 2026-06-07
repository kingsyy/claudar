<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Wizard from "./Wizard.svelte";

  type InstanceInfo = {
    name: string;
    has_session: boolean;
  };

  let instances = $state<InstanceInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // "Add account" flow
  let addingName = $state(false);
  let newName = $state("");
  let addError = $state<string | null>(null);
  let adding = $state(false);
  let wizardInstance = $state<string | null>(null);

  // "Remove" flow
  let pendingRemoval = $state<string | null>(null);
  let removing = $state(false);
  let removeError = $state<string | null>(null);

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
      await invoke("add_instance", { name });
      addingName = false;
      wizardInstance = name;
    } catch (e) {
      addError = String(e);
    } finally {
      adding = false;
    }
  }

  async function finishWizard() {
    wizardInstance = null;
    await loadInstances();
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

{#if wizardInstance}
  <Wizard instance={wizardInstance} onComplete={finishWizard} />
{:else}
  <div class="page">
    <div class="header-row">
      <div>
        <h1>Accounts</h1>
        <p class="subtitle">Manage the Claude.ai accounts Claude Notify monitors.</p>
      </div>
      <button class="primary" onclick={startAddAccount} disabled={addingName}>
        + Add account
      </button>
    </div>

    {#if addingName}
      <div class="add-card">
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
      <ul class="account-list">
        {#each instances as inst (inst.name)}
          <li class="account-row">
            <div class="account-info">
              <span class="account-name">{inst.name}</span>
              <span class="account-status" class:active={inst.has_session}>
                <span class="status-dot" aria-hidden="true"></span>
                {inst.has_session ? "Active" : "No session"}
              </span>
            </div>
            <button class="danger" onclick={() => requestRemoval(inst.name)}>Remove</button>
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
    color: hsl(222.2 84% 4.9%);
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
    color: hsl(222.2 84% 4.9%);
  }

  .subtitle {
    color: hsl(215.4 16.3% 46.9%);
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
    background-color: hsl(222.2 84% 4.9%);
    color: hsl(210 40% 98%);
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
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    background: none;
    color: hsl(222.2 84% 4.9%);
    font-size: 0.85rem;
    cursor: pointer;
  }

  .danger {
    padding: 0.4rem 0.85rem;
    border: 1px solid hsl(0 70% 85%);
    border-radius: var(--radius);
    background: none;
    color: hsl(0 70% 45%);
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .danger:hover {
    background-color: hsl(0 70% 97%);
  }

  .danger:disabled,
  .ghost:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .add-card {
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    padding: 1rem;
    margin-bottom: 1.5rem;
    background-color: hsl(210 40% 98%);
  }

  .add-card label {
    display: block;
    font-size: 0.8rem;
    font-weight: 600;
    color: hsl(222.2 84% 4.9%);
    margin-bottom: 0.4rem;
  }

  .add-row {
    display: flex;
    gap: 0.5rem;
  }

  .add-row input {
    flex: 1;
    padding: 0.5rem 0.7rem;
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-family: inherit;
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

  .empty-state {
    padding: 3rem 1rem;
    text-align: center;
    color: hsl(215.4 16.3% 46.9%);
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
    border: 1px solid hsl(214.3 31.8% 91.4%);
    border-radius: var(--radius);
  }

  .account-info {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .account-name {
    font-size: 0.95rem;
    font-weight: 600;
    color: hsl(222.2 84% 4.9%);
  }

  .account-status {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.8rem;
    color: hsl(215.4 16.3% 46.9%);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: hsl(0 70% 60%);
  }

  .account-status.active .status-dot {
    background-color: hsl(142 71% 45%);
  }

  .account-status.active {
    color: hsl(142 71% 35%);
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
    background-color: hsl(0 0% 100%);
    border-radius: var(--radius);
    padding: 1.5rem;
    max-width: 420px;
    width: calc(100% - 2rem);
    box-shadow: 0 12px 40px hsl(222.2 84% 4.9% / 0.25);
  }

  .modal p {
    font-size: 0.875rem;
    color: hsl(215.4 16.3% 36%);
    line-height: 1.5;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }
</style>
