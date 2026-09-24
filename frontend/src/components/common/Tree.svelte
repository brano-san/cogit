<script lang="ts" generics="T extends import('$lib/tree').TreeNode">
  import Disclosure from "$components/common/Disclosure.svelte";
  import { flatten, toggle, type Flattened } from "$lib/tree";
  import type { Snippet } from "svelte";

  interface Props {
    nodes: readonly T[];
    collapsed: ReadonlySet<string>;
    oncollapse: (next: Set<string>) => void;
    /** Draws one row's content; the caret and the indent belong to this component. */
    row: Snippet<[Flattened<T>]>;
    label?: string;
  }

  let { nodes, collapsed, oncollapse, row, label }: Props = $props();

  const rows = $derived(flatten(nodes, collapsed));
</script>

<div class="tree tree-rows key-list" role="tree" aria-label={label}>
  {#each rows as node (node.id)}
    <div
      class="node"
      role="treeitem"
      aria-expanded={node.open}
      aria-selected="false"
      tabindex="-1"
      style:padding-left="calc(var(--tree-base) + {node.depth} * var(--tree-step))"
    >
      <Disclosure
        empty={node.open === undefined}
        open={node.open ?? false}
        label={node.open ? "Collapse" : "Expand"}
        onclick={() => oncollapse(toggle(collapsed, node.id))}
      />
      {@render row(node)}
    </div>
  {/each}
</div>

<style>
  /* Text follows the triangle here, so a child's triangle starts under the parent's text. */
  .tree {
    --tree-next: var(--disclosure-glyph);
  }

  .node {
    display: flex;
    align-items: center;
    gap: var(--tree-gap);
    height: var(--h-row-dense);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }
</style>
