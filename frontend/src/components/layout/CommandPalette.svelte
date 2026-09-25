<script lang="ts">
  import { modalLayer, modals } from "$lib/modal-stack";
  import { rankCommands, type PaletteCommand } from "$lib/palette";

  interface Props {
    commands: readonly PaletteCommand[];
    recent: readonly string[];
    onrun: (command: PaletteCommand) => void;
    onclose: () => void;
  }

  let { commands, recent, onrun, onclose }: Props = $props();

  let query = $state("");
  let cursor = $state(0);
  let field: HTMLInputElement | undefined = $state();

  const shown = $derived(rankCommands(commands, query, recent));

  /** A modal layer (11 §1): Esc closes it wherever the focus went inside it. */
  const layer = modalLayer();

  function onwindowkey(event: KeyboardEvent) {
    if (event.key !== "Escape" || !modals.isTop(layer) || event.defaultPrevented) return;
    event.preventDefault();
    onclose();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      cursor = Math.min(cursor + 1, shown.length - 1);
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      cursor = Math.max(cursor - 1, 0);
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const picked = shown[cursor];
      if (picked && !picked.unavailable) onrun(picked);
    }
  }

  $effect(() => {
    void query;
    cursor = 0;
  });

  $effect(() => {
    field?.focus();
  });
</script>

<svelte:window onkeydown={onwindowkey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="palette" role="dialog" aria-label="Find command">
  <input
    bind:this={field}
    bind:value={query}
    {onkeydown}
    type="text"
    placeholder="Find a command…"
    aria-label="Find a command"
  />

  {#if shown.length === 0}
    <p class="empty">Nothing matches “{query}”.</p>
  {:else}
    <div class="list">
      {#each shown as command, index (command.id)}
        <div
          class="row"
          class:active={index === cursor}
          class:blocked={Boolean(command.unavailable)}
          role="button"
          tabindex="-1"
          onmouseenter={() => (cursor = index)}
          onclick={() => !command.unavailable && onrun(command)}
          onkeydown={(e) => e.key === "Enter" && !command.unavailable && onrun(command)}
        >
          <span class="title truncate">{command.title}</span>
          {#if command.unavailable}
            <span class="why truncate">{command.unavailable}</span>
          {:else if command.shortcut}
            <span class="shortcut mono">{command.shortcut}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--scrim);
  }

  .palette {
    position: absolute;
    top: 12%;
    left: 50%;
    transform: translateX(-50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    width: min(560px, 80vw);
    max-height: 60vh;
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-dialog);
    overflow: hidden;
  }

  input {
    flex: 0 0 auto;
    height: 32px;
    padding: 0 var(--sp-5);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 0;
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-ui);
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: 26px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .row.active {
    background: var(--state-selected);
  }

  .row.blocked .title {
    color: var(--text-secondary);
  }

  .title {
    flex: 1 1 auto;
    min-width: 0;
  }

  .shortcut,
  .why {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .why {
    font-style: italic;
  }

  .empty {
    margin: 0;
    padding: var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
