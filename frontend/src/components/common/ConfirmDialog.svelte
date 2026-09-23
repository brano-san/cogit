<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";

  /** A yes/no question in the application's own modal. */
  interface Props {
    title: string;
    message: string;
    items?: readonly string[];
    confirm: string;
    danger?: boolean;
    onanswer: (yes: boolean) => void;
  }

  let { title, message, items = [], confirm, danger = false, onanswer }: Props = $props();

  const SHOWN = 12;
</script>

<Dialog {title} onclose={() => onanswer(false)} onconfirm={() => onanswer(true)}>
  <div class="body">
    <p>{message}</p>
    {#if items.length > 0}
      <ul class="items mono">
        {#each items.slice(0, SHOWN) as item (item)}<li class="truncate" title={item}>{item}</li>{/each}
        {#if items.length > SHOWN}<li class="more">and {items.length - SHOWN} more</li>{/if}
      </ul>
    {/if}
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={() => onanswer(false)}>Cancel</button>
    <button type="button" class="btn" class:primary={!danger} class:warning={danger} data-autofocus onclick={() => onanswer(true)}>
      {confirm}
    </button>
  {/snippet}
</Dialog>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    font-size: var(--fs-dense);
  }

  p {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .items {
    max-height: 14em;
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    overflow-y: auto;
    list-style: none;
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
  }

  .more {
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
