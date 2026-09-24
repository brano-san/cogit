<script lang="ts">
  import type { DropAction } from "$lib/drop-target";

  interface Props {
    actions: readonly DropAction[];
    x: number;
    y: number;
    onpick: (action: DropAction) => void;
    onclose: () => void;
  }

  let { actions, x, y, onpick, onclose }: Props = $props();

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="menu" role="menu" style:left="{x}px" style:top="{y}px">
  {#each actions as action (action.id)}
    <button
      type="button"
      role="menuitem"
      disabled={action.disabled !== undefined}
      title={action.disabled}
      onclick={() => onpick(action)}
    >
      {action.title}
      {#if action.destructive}<span class="mark" title="Rewrites history">rewrites</span>{/if}
    </button>
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: fixed;
    z-index: 31;
    display: flex;
    flex-direction: column;
    min-width: 220px;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
  }

  button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-5);
    height: var(--h-row);
    padding: 0 var(--sp-5);
    background: none;
    color: var(--text-primary);
    border: 0;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  button:disabled {
    opacity: 0.4;
  }

  .mark {
    color: var(--status-modify);
    font-size: 10px;
    text-transform: uppercase;
  }
</style>
