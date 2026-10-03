<script lang="ts" generics="T extends import('$lib/tree').TreeNode">
  import { tick, type Snippet } from "svelte";
  import Disclosure from "$components/common/Disclosure.svelte";
  import { flatten, treeKey, type Flattened } from "$lib/tree";

  interface Props {
    nodes: readonly T[];
    collapsed: ReadonlySet<string>;
    /** Opens or closes a node: the Right/Left keys, or `onactivate` deciding to. */
    ontoggle: (id: string) => void;
    /** A click on a row, or Enter / Space on the focused one. */
    onactivate: (id: string) => void;
    /** The row whose content is shown; `aria-selected` and the selection fill follow it. */
    selected?: string | null;
    /** Draws one row's content; the caret and the indent belong to this component. */
    row: Snippet<[Flattened<T>]>;
    label?: string;
  }

  let { nodes, collapsed, ontoggle, onactivate, selected = null, row, label }: Props = $props();

  const rows = $derived(flatten(nodes, collapsed));
  let focused = $state<string | null>(null);
  /** One row takes Tab (roving tabindex): the one last focused while it is still shown,
      else the selected one, else the first. */
  const focus = $derived(
    rows.find((node) => node.id === focused)?.id ??
      rows.find((node) => node.id === selected)?.id ??
      rows[0]?.id ??
      null,
  );
  let element = $state<HTMLElement>();

  async function moveTo(id: string) {
    focused = id;
    await tick();
    element?.querySelector<HTMLElement>(`[data-tree-id="${CSS.escape(id)}"]`)?.focus();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    const action = treeKey(rows, focus, event.key);
    if (!action) return;
    event.preventDefault();
    if (action.kind === "focus") void moveTo(action.id);
    else if (action.kind === "toggle") ontoggle(action.id);
    else onactivate(action.id);
  }
</script>

<div
  class="tree tree-rows key-list"
  role="tree"
  aria-label={label}
  bind:this={element}
>
  {#each rows as node (node.id)}
    <div
      class="node"
      role="treeitem"
      aria-level={node.depth + 1}
      aria-expanded={node.open}
      aria-selected={node.id === selected}
      tabindex={node.id === focus ? 0 : -1}
      data-tree-id={node.id}
      style:padding-left="calc(var(--tree-base) + {node.depth} * var(--tree-step))"
      onfocus={() => (focused = node.id)}
      onclick={() => onactivate(node.id)}
      {onkeydown}
    >
      <!-- The whole row toggles, so the triangle is a picture, not a second button. -->
      <Disclosure empty={node.open === undefined} open={node.open ?? false} />
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
    padding-right: var(--sp-4);
    font-size: var(--fs-dense);
    white-space: nowrap;
    cursor: default;
  }

  .node:hover {
    background: var(--state-hover);
  }

  /* Selection is the fill and the bar; keyboard focus on any other row is the ring
     (`:focus-visible` in app.css), so the two never look alike. */
  .node[aria-selected="true"] {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--selected-bar);
  }
</style>
