<script lang="ts">
  import { nextDateOrder } from "$lib/ref-sort";
  import { refs } from "$stores/refs.svelte";

  const natural = $derived(refs.sort.names === "natural");
  const dates = $derived(refs.sort.dates);
  const dateTitle = $derived(
    dates === "off"
      ? "Sort by tip date: off (click for newest first)"
      : dates === "newest"
        ? "Sort by tip date: newest first (click for oldest first)"
        : "Sort by tip date: oldest first (click to sort by name)",
  );
</script>

<button
  type="button"
  class="sort"
  class:on={natural}
  aria-pressed={natural}
  title={natural
    ? "Natural sort: numbers compare as numbers, v1.0.2 before v1.0.10"
    : "Plain sort: character by character, v1.0.10 before v1.0.2"}
  onclick={() => refs.setSort({ ...refs.sort, names: natural ? "plain" : "natural" })}
>
  <svg viewBox="0 0 16 16" aria-hidden="true"
    ><path d="M2.5 5.5 4 4v8M8 5a1.5 1.5 0 0 1 3 0c0 1.5-3 3-3 5h3M13.5 4v8M12 10.5l1.5 1.5 1.5-1.5" /></svg
  >
</button>
<button
  type="button"
  class="sort"
  class:on={dates !== "off"}
  aria-pressed={dates !== "off"}
  title={dateTitle}
  onclick={() => refs.setSort({ ...refs.sort, dates: nextDateOrder(dates) })}
>
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <circle cx="6.5" cy="8" r="4.5" />
    <path d="M6.5 5.5V8l1.5 1" />
    {#if dates === "oldest"}
      <path d="M13.5 12V4M12 5.5 13.5 4 15 5.5" />
    {:else}
      <path d="M13.5 4v8M12 10.5l1.5 1.5 1.5-1.5" />
    {/if}
  </svg>
</button>

<style>
  .sort {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    width: var(--h-button-sm);
    height: var(--h-button-sm);
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    cursor: default;
  }

  .sort:hover {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .sort.on {
    background: var(--state-selected);
    color: var(--status-ref);
  }

  svg {
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
