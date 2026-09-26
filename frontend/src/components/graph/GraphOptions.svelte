<script lang="ts">
  import { tick } from "svelte";
  import { graphOptions, type GraphOptionEntry } from "$lib/graph-options";
  import { menuKey } from "$lib/menu-keys";
  import { settings } from "$stores/settings.svelte";

  /** The graph's options, beside its filter (F-561): SmartGit's hamburger menu. */
  interface Props {
    onpreferences: () => void;
  }

  let { onpreferences }: Props = $props();

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let button: HTMLButtonElement | undefined = $state();

  const entries = $derived(graphOptions(settings.current));

  function toggle(event: MouseEvent) {
    open = !open;
    if (open && event.detail === 0) void tick().then(() => press("Home"));
  }

  function close(refocus: boolean) {
    open = false;
    if (refocus) button?.focus();
  }

  /** Whether the key was the menu's. */
  function press(key: string): boolean {
    const items = [...(root?.querySelectorAll<HTMLButtonElement>(".menu [role^='menuitem']") ?? [])];
    const at = items.findIndex((item) => item === document.activeElement);
    const move = menuKey(key, at < 0 ? null : at, items.map(() => true));
    if (move === null) return false;
    if (move.kind === "focus") {
      items[move.to]?.focus();
      return true;
    }
    close(key !== "Tab");
    return key !== "Tab";
  }

  function onwindowkey(event: KeyboardEvent) {
    if (open && !event.defaultPrevented && press(event.key)) event.preventDefault();
  }

  /** A switch stays open so several can be set in a row; the link closes the menu. */
  function run(entry: GraphOptionEntry) {
    if (entry.kind === "coloring") void settings.set("graphColoring", entry.coloring);
    if (entry.kind === "switch") void settings.set(entry.key, !entry.checked);
    if (entry.kind === "preferences") {
      close(false);
      onpreferences();
    }
  }
</script>

<svelte:window onkeydown={onwindowkey} />

<div class="options" bind:this={root}>
  <button
    type="button"
    class="tool"
    class:open
    bind:this={button}
    aria-haspopup="menu"
    aria-expanded={open}
    title="Graph options"
    aria-label="Graph options"
    onclick={toggle}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 4h11M2.5 8h11M2.5 12h11" /></svg>
  </button>
  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="backdrop" onclick={() => close(false)}></div>
    <div class="menu" role="menu" aria-label="Graph options">
      {#each entries as entry, index (index)}
        {#if entry.kind === "separator"}
          <div class="separator" role="separator"></div>
        {:else if entry.kind === "preferences"}
          <button type="button" role="menuitem" title={entry.hint} onclick={() => run(entry)}>
            <span class="mark" aria-hidden="true"></span>{entry.label}
          </button>
        {:else}
          <button
            type="button"
            role={entry.kind === "coloring" ? "menuitemradio" : "menuitemcheckbox"}
            aria-checked={entry.checked}
            title={entry.hint}
            onclick={() => run(entry)}
          >
            <span class="mark" aria-hidden="true">{entry.checked ? (entry.kind === "coloring" ? "●" : "✓") : ""}</span
            >{entry.label}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .options {
    position: relative;
    display: flex;
    flex: 0 0 auto;
  }

  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--h-button-sm);
    height: var(--h-button-sm);
    padding: 0;
    background: none;
    color: var(--text-secondary);
    border: 0;
    border-radius: var(--r-sm);
    cursor: default;
  }

  .tool:hover,
  .tool.open {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  svg {
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: absolute;
    top: calc(100% + var(--sp-1));
    right: 0;
    z-index: 31;
    display: flex;
    flex-direction: column;
    min-width: 260px;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
  }

  [role^="menuitem"] {
    display: flex;
    align-items: center;
    height: var(--h-row);
    padding: 0 var(--sp-5) 0 var(--sp-3);
    background: none;
    color: var(--text-primary);
    border: 0;
    font-size: var(--fs-dense);
    text-align: left;
    white-space: nowrap;
    cursor: default;
  }

  [role^="menuitem"]:hover,
  [role^="menuitem"]:focus-visible {
    background: var(--state-hover);
    outline: none;
  }

  .mark {
    flex: 0 0 auto;
    width: 18px;
    color: var(--status-ref);
    text-align: center;
  }

  .separator {
    height: 1px;
    margin: var(--sp-2) 0;
    background: var(--divider);
  }
</style>
