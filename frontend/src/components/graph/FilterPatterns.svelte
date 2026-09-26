<script lang="ts">
  import { tick } from "svelte";
  import { forgetBlocked, rememberBlocked } from "$lib/filter-patterns";
  import { menuKey } from "$lib/menu-keys";
  import { graphFilter } from "$stores/graph-filter.svelte";

  /** The magnifier in the graph filter: SmartGit's Remember Pattern and the patterns kept
      by it (F-563). */
  interface Props {
    /** A remembered pattern was chosen: it goes into the field and filters at once. */
    onpick: (pattern: string) => void;
  }

  let { onpick }: Props = $props();

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let lens: HTMLButtonElement | undefined = $state();

  const rememberOff = $derived(rememberBlocked(graphFilter.patterns, graphFilter.text));
  const forgetOff = $derived(forgetBlocked(graphFilter.patterns, graphFilter.text));

  function toggle(event: MouseEvent) {
    open = !open;
    // Enter or Space on the button: the menu is walked from the keyboard.
    if (open && event.detail === 0) void tick().then(() => press("Home"));
  }

  function close(refocus: boolean) {
    open = false;
    if (refocus) lens?.focus();
  }

  function items(): HTMLButtonElement[] {
    return [...(root?.querySelectorAll<HTMLButtonElement>(".menu [role^='menuitem']") ?? [])];
  }

  /** Whether the key was the menu's. */
  function press(key: string): boolean {
    const all = items();
    const at = all.findIndex((item) => item === document.activeElement);
    const move = menuKey(key, at < 0 ? null : at, all.map((item) => item.getAttribute("aria-disabled") !== "true"));
    if (move === null) return false;
    if (move.kind === "focus") {
      all[move.to]?.focus();
      return true;
    }
    close(key !== "Tab");
    return key !== "Tab";
  }

  function onwindowkey(event: KeyboardEvent) {
    if (!open || event.defaultPrevented) return;
    const focused = document.activeElement as HTMLElement | null;
    const pattern = focused?.dataset.pattern;
    if (event.key === "Delete" && pattern !== undefined) {
      event.preventDefault();
      graphFilter.forget(pattern);
      void tick().then(() => press("Home"));
      return;
    }
    if (press(event.key)) event.preventDefault();
  }

  function pick(pattern: string) {
    close(false);
    onpick(pattern);
  }

  function rememberNow() {
    if (rememberOff !== null) return;
    graphFilter.remember(graphFilter.text);
    close(true);
  }

  function forgetNow() {
    if (forgetOff !== null) return;
    graphFilter.forget(graphFilter.text);
    close(true);
  }
</script>

<svelte:window onkeydown={onwindowkey} />

<div class="patterns" bind:this={root}>
  <button
    type="button"
    class="lens"
    bind:this={lens}
    class:open
    aria-haspopup="menu"
    aria-expanded={open}
    title="Remembered filters"
    aria-label="Remembered filters"
    onclick={toggle}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="6.5" cy="6.5" r="4" /><path d="m9.5 9.5 4 4" /></svg>
  </button>
  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="backdrop" onclick={() => close(false)}></div>
    <div class="menu" role="menu" aria-label="Remembered filters">
      <button
        type="button"
        role="menuitem"
        aria-disabled={rememberOff !== null}
        class:dead={rememberOff !== null}
        title={rememberOff ?? "Keep the text in the field for later"}
        onclick={rememberNow}>Remember Pattern</button
      >
      <button
        type="button"
        role="menuitem"
        aria-disabled={forgetOff !== null}
        class:dead={forgetOff !== null}
        title={forgetOff ?? "Drop the text in the field from the remembered ones"}
        onclick={forgetNow}>Forget Pattern</button
      >
      {#if graphFilter.patterns.length > 0}
        <div class="separator" role="separator"></div>
        {#each graphFilter.patterns as pattern (pattern)}
          <div class="saved">
            <button
              type="button"
              role="menuitem"
              class="pattern"
              data-pattern={pattern}
              title="{pattern} — Delete forgets it"
              onclick={() => pick(pattern)}><span class="truncate">{pattern}</span></button
            >
            <button
              type="button"
              class="forget"
              tabindex="-1"
              title="Forget this pattern"
              aria-label="Forget {pattern}"
              onclick={() => graphFilter.forget(pattern)}>✕</button
            >
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .patterns {
    position: relative;
    display: flex;
    flex: 0 0 auto;
  }

  .lens {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: calc(var(--h-button-sm) - 2px);
    height: calc(var(--h-button-sm) - 2px);
    padding: 0;
    background: none;
    color: var(--text-secondary);
    border: 0;
    border-radius: var(--r-sm);
    cursor: default;
  }

  .lens:hover,
  .lens.open {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  svg {
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: absolute;
    top: calc(100% + var(--sp-1));
    left: 0;
    z-index: 31;
    display: flex;
    flex-direction: column;
    min-width: 220px;
    max-width: 420px;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
  }

  [role="menuitem"] {
    display: flex;
    align-items: center;
    min-width: 0;
    height: var(--h-row);
    padding: 0 var(--sp-5);
    background: none;
    color: var(--text-primary);
    border: 0;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  [role="menuitem"]:hover:not(.dead),
  [role="menuitem"]:focus-visible {
    background: var(--state-hover);
    outline: none;
  }

  .dead {
    opacity: 0.4;
  }

  .separator {
    height: 1px;
    margin: var(--sp-2) 0;
    background: var(--divider);
  }

  .saved {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .pattern {
    flex: 1 1 auto;
    font-family: var(--font-mono);
  }

  .forget {
    flex: 0 0 auto;
    height: var(--h-row);
    padding: 0 var(--sp-4);
    background: none;
    color: var(--text-secondary);
    border: 0;
    cursor: default;
  }

  .forget:hover {
    color: var(--text-primary);
  }
</style>
