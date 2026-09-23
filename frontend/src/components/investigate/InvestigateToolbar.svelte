<script lang="ts">
  import Caret from "$components/common/Caret.svelte";
  import { backList, locationLabel } from "$lib/investigate/history";
  import { PERSPECTIVES } from "$lib/investigate/perspectives";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";

  interface Props {
    session: InvestigateSession;
  }

  let { session }: Props = $props();

  let listOpen = $state(false);
  const earlier = $derived(backList(session.history));

  function jump(index: number) {
    listOpen = false;
    void session.jump(index);
  }
</script>

{#if listOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={() => (listOpen = false)}></div>
{/if}

<div class="toolbar">
  <div class="split">
    <button
      type="button"
      disabled={!session.canGoBack}
      title="Back (Alt+Left)"
      onclick={() => void session.back()}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M10 3 5 8l5 5" /></svg>
      Back
    </button>
    <button
      type="button"
      class="more"
      disabled={!session.canGoBack}
      aria-label="Places to go back to"
      title="Places to go back to"
      aria-expanded={listOpen}
      onclick={() => (listOpen = !listOpen)}><Caret open={listOpen} /></button
    >
    {#if listOpen}
      <div class="history" role="menu" aria-label="Back history">
        {#each earlier as entry (entry.index)}
          <button type="button" role="menuitem" onclick={() => jump(entry.index)}
            >{locationLabel(entry.location)}</button
          >
        {/each}
      </div>
    {/if}
  </div>
  <button
    type="button"
    disabled={!session.canGoForward}
    title="Forward (Alt+Right)"
    onclick={() => void session.forward()}
  >
    Forward
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m6 3 5 5-5 5" /></svg>
  </button>

  <span class="grow"></span>

  <div class="perspectives" role="tablist" aria-label="Perspective">
    {#each PERSPECTIVES as perspective (perspective.id)}
      <button
        type="button"
        role="tab"
        aria-selected={session.perspective === perspective.id}
        class:active={session.perspective === perspective.id}
        title="{perspective.hint} ({perspective.shortcut})"
        onclick={() => session.setPerspective(perspective.id)}>{perspective.label}</button
      >
    {/each}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .toolbar {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-toolbar);
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  button {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    height: var(--h-button);
    padding: 0 var(--sp-4);
    background: none;
    color: var(--text-primary);
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    font: inherit;
    cursor: default;
  }

  button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  button:disabled {
    color: var(--text-secondary);
    opacity: 0.55;
  }

  svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .split {
    position: relative;
    display: flex;
  }

  .more {
    padding: 0 var(--sp-2);
  }

  .history {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 31;
    display: flex;
    flex-direction: column;
    min-width: 280px;
    max-height: 360px;
    overflow-y: auto;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
  }

  .history button {
    justify-content: flex-start;
    height: var(--h-row);
    border-radius: 0;
    white-space: nowrap;
  }

  .grow {
    flex: 1 1 auto;
  }

  .perspectives {
    display: flex;
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    overflow: hidden;
  }

  .perspectives button {
    border: 0;
    border-radius: 0;
  }

  .perspectives button + button {
    border-left: 1px solid var(--field-border);
  }

  .perspectives button.active {
    background: var(--state-selected);
    color: var(--text-primary);
  }
</style>
