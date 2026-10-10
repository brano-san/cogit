<script lang="ts">
  import type { Snippet } from "svelte";
  import Disclosure from "$components/common/Disclosure.svelte";

  /** A count that opens into its list: the commits and the tags of the Push dialog. */
  interface Props {
    label: string;
    /** Nothing to list: the row does not open. */
    empty: boolean;
    open: boolean;
    ontoggle: () => void;
    children: Snippet;
  }

  let { label, empty, open, ontoggle, children }: Props = $props();
</script>

<div class="wrap">
  <button type="button" class="toggle" aria-expanded={open} disabled={empty} onclick={ontoggle}>
    <Disclosure {open} {empty} />
    <span>{label}</span>
  </button>
  {#if open && !empty}
    <ul class="list">{@render children()}</ul>
  {/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    max-height: 180px;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    overflow-y: auto;
    background: var(--bg-panel);
    border-radius: var(--r-sm);
    list-style: none;
    font-size: var(--fs-dense);
  }
</style>
