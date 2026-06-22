<script lang="ts">
  // Accessible on/off toggle. State is conveyed four redundant ways — text
  // ("On"/"Off"), icon (✓/✕), colour, and thumb position — so it stays legible
  // for colourblind users and doesn't depend on left-vs-right meaning.
  let {
    checked = false,
    disabled = false,
    label,
    onToggle,
  }: {
    checked?: boolean;
    disabled?: boolean;
    label: string;
    onToggle: () => void;
  } = $props();
</script>

<button
  type="button"
  class="switch"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => {
    if (!disabled) onToggle();
  }}
>
  <span class="switch-text" aria-hidden="true">{checked ? "On" : "Off"}</span>
  <span class="switch-thumb" aria-hidden="true">{checked ? "✓" : "✕"}</span>
</button>

<style>
  .switch {
    position: relative;
    width: 68px;
    height: 28px;
    border-radius: 999px;
    border: 1px solid hsl(var(--border));
    background-color: hsl(var(--muted));
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
    transition:
      background-color 0.15s,
      border-color 0.15s;
  }

  .switch:focus-visible {
    outline: 2px solid hsl(var(--ring));
    outline-offset: 2px;
  }

  .switch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .switch.on {
    background-color: hsl(var(--switch-on));
    border-color: hsl(var(--switch-on));
  }

  .switch-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background-color: hsl(0 0% 100%);
    color: hsl(var(--muted-foreground));
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.7rem;
    font-weight: 700;
    line-height: 1;
    box-shadow: 0 1px 2px hsl(222.2 84% 4.9% / 0.25);
    transition: transform 0.15s;
  }

  .switch.on .switch-thumb {
    transform: translateX(40px);
    color: hsl(var(--switch-on));
  }

  .switch-text {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    line-height: 1;
    color: hsl(var(--muted-foreground));
  }

  /* Thumb left when off → label sits on the right; mirror when on. */
  .switch:not(.on) .switch-text {
    right: 9px;
  }

  .switch.on .switch-text {
    left: 11px;
    color: hsl(0 0% 100%);
  }
</style>
