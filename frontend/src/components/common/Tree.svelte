<script lang="ts" generics="T extends import('$lib/tree').TreeNode">
  import { flatten, toggle, type Flattened } from "$lib/tree";
  import type { Snippet } from "svelte";

  interface Props {
    nodes: readonly T[];
    collapsed: ReadonlySet<string>;
    oncollapse: (next: Set<string>) => void;
    /** Draws one row's content; the caret and the indent belong to this component. */
    row: Snippet<[Flattened<T>]>;
    indent?: number;
    /** Left padding of the shallowest row, so the caret never touches the panel edge. */
    base?: number;
    label?: string;
  }

  let { nodes, collapsed, oncollapse, row, indent = 12, base = 8, label }: Props = $props();

  const rows = $derived(flatten(nodes, collapsed));
</script>

<div class="tree" role="tree" aria-label={label}>
  {#each rows as node (node.id)}
    <div
      class="node"
      role="treeitem"
      aria-expanded={node.open}
      aria-selected="false"
      tabindex="-1"
      style:padding-left="{base + node.depth * indent}px"
    >
      {#if node.open === undefined}
        <span class="caret spacer" aria-hidden="true"></span>
      {:else}
        <button
          type="button"
          class="caret"
          aria-label={node.open ? "Collapse" : "Expand"}
          onclick={() => oncollapse(toggle(collapsed, node.id))}>{node.open ? "▾" : "▸"}</button
        >
      {/if}
      {@render row(node)}
    </div>
  {/each}
</div>

<style>
  .node {
    display: flex;
    align-items: center;
    gap: var(--sp-2, 3px);
    height: var(--h-row-dense);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .caret {
    flex: 0 0 12px;
    width: 12px;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    line-height: 1;
    text-align: center;
    cursor: default;
  }

  .caret.spacer {
    display: inline-block;
  }
</style>
