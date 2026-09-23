<script lang="ts">
  import Disclosure from "$components/common/Disclosure.svelte";
  import KindIcon from "$components/common/KindIcon.svelte";
  import {
    buildRefTree,
    checkState,
    foldedWhileFiltering,
    leavesUnder,
    toggleNode,
    type RefNode,
    type RefTreeInput,
  } from "$lib/ref-nodes";
  import { DRAG_TYPE, parseDrag, serialiseDrag } from "$lib/drop-target";
  import { flatten } from "$lib/tree";
  import { triState } from "$lib/tri-state-box";
  import { worktreeMarkTooltip } from "$lib/worktree-list";
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
  const nodes = $derived(flatten(tree, foldedWhileFiltering(input.collapsed, input.filter)));

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

<div class="tree tree-rows" role="tree" aria-label="References">
  {#each nodes as node (node.id)}
    {@const state = checkState(tree, node.id, visible)}
    {@const tickable = leavesUnder(tree, node.id).length > 0}
    <div
      class="row {node.kind}"
      class:selected={active === node.id}
      class:over={over === node.id}
      style:padding-left="calc(var(--tree-base) + {node.depth} * var(--tree-step))"
      role="treeitem"
      aria-selected={active === node.id}
      aria-expanded={foldable(node) ? !input.collapsed.has(node.id) : undefined}
      tabindex="-1"
      title={node.branch?.name ?? node.tag?.name ?? node.detail ?? node.label}
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
      <Disclosure
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

      {#if node.kind === "folder"}<KindIcon kind="directory" title="Folder" />{/if}

      <button
        type="button"
        class="label truncate shrink-last"
        class:current={node.current}
        onclick={() => pick(node)}
        ondblclick={() => {
          if (node.branch?.kind === "local") oncheckout?.(node.branch);
          else onactivate?.(node);
        }}>{node.label}</button
      >

      {#if node.worktree}
        <KindIcon kind="worktree" title={worktreeMarkTooltip(node.worktree)} />
        <span class="wt-state {node.worktree.state}" title={worktreeMarkTooltip(node.worktree)}
          >{node.worktree.state}</span
        >
      {/if}

      {#if node.detail}<span class="detail truncate shrink-first">{node.detail}</span>{/if}
    </div>
  {/each}
</div>

<style>
  .tree {
    --ref-box: 12px;
    --tree-next: var(--ref-box);
    padding: var(--sp-3) 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--tree-gap);
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
    width: var(--ref-box);
    height: var(--ref-box);
    margin: 0;
    accent-color: var(--status-ref);
  }

  .label.current {
    color: var(--status-ref);
    font-weight: 600;
  }

  .wt-state {
    flex: 0 0 auto;
    padding: 0 var(--sp-2);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    font-size: 10px;
    line-height: 12px;
  }

  .wt-state.changes {
    color: var(--status-modify);
    border-color: var(--status-modify);
  }

  .wt-state.missing {
    color: var(--status-delete);
    border-color: var(--status-delete);
  }

  .label {
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
    flex-grow: 1;
    text-align: right;
    color: var(--text-secondary);
    font-size: 10px;
    text-transform: none;
    letter-spacing: 0;
  }
</style>
