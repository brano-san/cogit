<script lang="ts">
  import { shortOid } from "$lib/format";
  import type { Tag } from "$lib/ipc";

  interface Props {
    tags: readonly Tag[];
    oncheckout: (tag: Tag) => void;
    ondelete: (tag: Tag) => void;
  }

  let { tags, oncheckout, ondelete }: Props = $props();
</script>

{#if tags.length > 0}
  <div class="section">
    <div class="section-header">Tags ({tags.length})</div>
    {#each tags as tag (tag.fullName)}
      <div class="row" title={tag.isAnnotated ? `${tag.name} (annotated)` : tag.name}>
        <span class="marker" aria-hidden="true">{tag.isAnnotated ? "◆" : "◇"}</span>
        <span class="name truncate">{tag.name}</span>
        <span class="oid mono tabular">{shortOid(tag.oid)}</span>
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Check out {tag.name}"
          onclick={() => oncheckout(tag)}
          onkeydown={(e) => e.key === "Enter" && oncheckout(tag)}>Checkout</span
        >
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Delete {tag.name}"
          onclick={() => ondelete(tag)}
          onkeydown={(e) => e.key === "Enter" && ondelete(tag)}>Delete</span
        >
      </div>
    {/each}
  </div>
{/if}

<style>
  .section {
    padding-bottom: var(--sp-4);
  }

  .section-header {
    padding: var(--sp-3) var(--sp-5) var(--sp-2, 3px);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .marker {
    flex: 0 0 auto;
    width: 10px;
    color: var(--status-stash);
  }

  .name {
    flex: 1 1 auto;
    min-width: 0;
  }

  .oid {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-3);
    color: var(--text-secondary);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0;
    cursor: default;
  }

  .row:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-ref);
  }
</style>
