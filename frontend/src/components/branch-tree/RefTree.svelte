<script lang="ts">
  import Caret from "$components/common/Caret.svelte";
  import {
    buildRefTree,
    checkState,
    leavesUnder,
    toggleNode,
    type RefNode,
    type RefTreeInput,
  } from "$lib/ref-nodes";
  import { DRAG_TYPE, parseDrag, serialiseDrag } from "$lib/drop-target";
  import { flatten } from "$lib/tree";
  import { triState } from "$lib/tri-state-box";
  import type { Branch } from "$lib/ipc";

  interface Props {
    input: RefTreeInput;
    visible: ReadonlySet<string>;
    onvisible: (next: Set<string>) => void;
    oncollapse: (id: string) => void;
    /** Clicking the text, not the box: select the ref and centre the graph on its tip. */
    onselect?: (node: RefNode) => void;
    oncheckout?: (branch: Branch) => void;
    onactivate?: (node: RefNode) => void;
    oncontext?: (node: RefNode, x: number, y: number) => void;
    ondrop?: (source: string, target: Branch) => void;
  }

  let {
    input,
    visible,
    onvisible,
    oncollapse,
    onselect,
    oncheckout,
    onactivate,
    oncontext,
    ondrop,
  }: Props = $props();

  let over = $state<string | null>(null);
  let active = $state<string | null>(null);

  /** Every row, folded or not: a heading's box is counted from this, never from the rows
      on screen, so folding a group cannot take its ticks away (R-158). */
  const tree = $derived(buildRefTree(input));
  const nodes = $derived(flatten(tree, input.collapsed));

  function toggle(id: string) {
    const next = toggleNode(tree, id, visible);
    onvisible(next);
    return checkState(tree, id, next);
  }

  function foldable(node: RefNode): boolean {
    return node.children === true;
  }

  function pick(node: RefNode) {
    active = node.id;
    if (node.branch) onselect?.(node);
    else if (node.rev) onselect?.(node);
  }

  function dropped(event: DragEvent, target: Branch) {
    over = null;
    const payload = parseDrag(event.dataTransfer?.getData(DRAG_TYPE) ?? "");
    if (payload?.kind === "branch" && payload.id !== target.name) ondrop?.(payload.id, target);
  }
</script>

<div class="tree" role="tree" aria-label="References">
  {#each nodes as node (node.id)}
    {@const state = checkState(tree, node.id, visible)}
    {@const tickable = leavesUnder(tree, node.id).length > 0}
    <div
      class="row {node.kind}"
      class:selected={active === node.id}
      class:over={over === node.id}
      style:padding-left="calc(var(--sp-4) + {node.depth * 12}px)"
      role="treeitem"
      aria-selected={active === node.id}
      aria-expanded={foldable(node) ? !input.collapsed.has(node.id) : undefined}
      tabindex="-1"
      title={node.detail ?? node.label}
      draggable={node.branch !== undefined && ondrop !== undefined}
      ondragstart={(event) =>
        node.branch &&
        event.dataTransfer?.setData(
          DRAG_TYPE,
          serialiseDrag({ kind: "branch", id: node.branch.name }),
        )}
      ondragover={(event) => {
        if (ondrop && node.branch) {
          event.preventDefault();
          over = node.id;
        }
      }}
      ondragleave={() => (over = null)}
      ondrop={(event) => node.branch && dropped(event, node.branch)}
      oncontextmenu={(event) => {
        if (!oncontext) return;
        event.preventDefault();
        oncontext(node, event.clientX, event.clientY);
      }}
    >
      <Caret
        empty={!foldable(node)}
        open={!input.collapsed.has(node.id)}
        label="Collapse {node.label}"
        onclick={() => oncollapse(node.id)}
      />

      <input
        type="checkbox"
        class="box"
        use:triState={{ state, toggle: () => toggle(node.id) }}
        disabled={!tickable}
        title={node.disabled}
        aria-label="Show {node.label} in the graph"
      />

      {#if node.marker}<span class="marker" aria-hidden="true">{node.marker}</span>{/if}

      <button
        type="button"
        class="label truncate"
        onclick={() => pick(node)}
        ondblclick={() => {
          if (node.branch?.kind === "local") oncheckout?.(node.branch);
          else onactivate?.(node);
        }}>{node.label}</button
      >

      {#if node.detail}<span class="detail truncate">{node.detail}</span>{/if}
    </div>
  {/each}
</div>

<style>
  .tree {
    padding: var(--sp-3) 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    height: var(--h-row-dense);
    padding-right: var(--sp-4);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .row.over {
    box-shadow: inset 0 0 0 1px var(--status-ref);
  }

  .row.group,
  .row.remote-group {
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.03em;
  }

  .box {
    flex: 0 0 auto;
    width: 12px;
    height: 12px;
    margin: 0;
    accent-color: var(--status-ref);
  }

  .marker {
    flex: 0 0 auto;
    color: var(--status-ref);
    font-size: 9px;
  }

  .label {
    flex: 0 1 auto;
    min-width: 0;
    background: none;
    border: 0;
    padding: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .row.head .label {
    color: var(--status-ref);
    font-weight: 600;
  }

  .row.stash .label {
    color: var(--status-stash);
  }

  .row.lost .label {
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }

  .detail {
    flex: 1 1 auto;
    min-width: 0;
    text-align: right;
    color: var(--text-secondary);
    font-size: 10px;
    text-transform: none;
    letter-spacing: 0;
  }
</style>
