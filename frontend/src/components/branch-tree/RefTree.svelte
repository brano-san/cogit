<script lang="ts">
  import Disclosure from "$components/common/Disclosure.svelte";
  import KindIcon from "$components/common/KindIcon.svelte";
  import {
    buildRefTree,
    checkState,
    foldedWhileFiltering,
    tickStates,
    toggleNode,
    type RefNode,
    type RefTreeInput,
  } from "$lib/ref-nodes";
  import { pointerDrag } from "$lib/pointer-drag";
  import { TypeAhead, findTyped, listKey, pageRows, pressOf, typedChar } from "$lib/list-keys";
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
    /** `x` and `y`: where the pointer was released, for the menu of what to do. */
    ondrop?: (source: string, target: Branch, x: number, y: number) => void;
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
  const ticks = $derived(tickStates(tree, visible));

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

  function dropped(source: string, target: string, x: number, y: number) {
    const from = nodes.find((node) => node.id === source)?.branch;
    const onto = nodes.find((node) => node.id === target)?.branch;
    if (from && onto && from.name !== onto.name) ondrop?.(from.name, onto, x, y);
  }

  /** A branch dropped on a branch (R-450); rows are marked with their node id. */
  const branchDrag = $derived(ondrop ? { onover: (id: string | null) => (over = id), ondrop: dropped } : null);

  let treeEl: HTMLDivElement | undefined = $state();
  const typing = new TypeAhead();

  /** A double-click: checkout for a local branch, the node's own action for the rest. */
  function activate(node: RefNode) {
    if (node.branch?.kind === "local") oncheckout?.(node.branch);
    else onactivate?.(node);
  }

  function goTo(index: number) {
    const node = nodes[index];
    if (!node) return;
    pick(node);
    treeEl?.querySelector<HTMLElement>(`[data-node="${CSS.escape(node.id)}"]`)?.focus();
  }

  function fold(at: number, open: boolean) {
    const node = nodes[at];
    if (!node) return;
    if (foldable(node) && input.collapsed.has(node.id) === open) {
      oncollapse(node.id);
      return;
    }
    // ← on a leaf, or on a folded heading, goes up to the heading it is under.
    if (open) return;
    const parent = nodes.slice(0, at).findLastIndex((above) => above.depth < node.depth);
    if (parent >= 0) goTo(parent);
  }

  /** 11 §10. The fold triangle and the tick box keep their own Enter and Space. */
  function onkeydown(event: KeyboardEvent) {
    const onControl = event.target instanceof HTMLElement && event.target.closest("button:not(.label), input");
    const press = pressOf(event);
    const found = nodes.findIndex((node) => node.id === active);
    const at = found === -1 ? null : found;
    const char = typedChar(press);
    if (char !== null) {
      const to = findTyped(nodes.map((node) => node.label), at, typing.type(char, event.timeStamp));
      if (to === null) return;
      event.preventDefault();
      goTo(to);
      return;
    }
    const row = treeEl?.querySelector<HTMLElement>(".row");
    const action = listKey(press, at, nodes.length, pageRows(treeEl?.parentElement, row?.offsetHeight ?? 22));
    if (action === null || (action.kind === "activate" && onControl)) return;
    event.preventDefault();
    if (action.kind === "move") goTo(action.to);
    else if (at === null) return;
    else if (action.kind === "fold") fold(at, action.open);
    else if (nodes[at]) activate(nodes[at]);
  }
</script>

<!-- Roving focus: the rows take it one at a time, the tree itself never does. -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div
  class="tree tree-rows key-list"
  role="tree"
  aria-label="References"
  bind:this={treeEl}
  use:pointerDrag={branchDrag}
  {onkeydown}
>
  {#each nodes as node (node.id)}
    {@const tick = ticks.get(node.id)}
    <div
      class="row {node.kind}"
      class:selected={active === node.id}
      class:over={over === node.id}
      style:padding-left="calc(var(--tree-base) + {node.depth} * var(--tree-step))"
      role="treeitem"
      aria-selected={active === node.id}
      aria-expanded={foldable(node) ? !input.collapsed.has(node.id) : undefined}
      tabindex="-1"
      data-node={node.id}
      title={node.branch?.name ?? node.tag?.name ?? node.detail ?? node.label}
      data-drag={node.branch && ondrop ? node.id : undefined}
      data-drop={node.branch && ondrop ? node.id : undefined}
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
        use:triState={{ state: tick?.state ?? "off", toggle: () => toggle(node.id) }}
        disabled={!tick?.tickable}
        title={node.disabled}
        aria-label="Show {node.label} in the graph"
      />

      {#if node.kind === "folder"}<KindIcon kind="directory" title="Folder" />{/if}

      <button
        type="button"
        class="label truncate shrink-last"
        class:current={node.current}
        onclick={() => pick(node)}
        ondblclick={() => activate(node)}>{node.label}</button
      >

      {#if node.worktree}
        <span class="wt-mark {node.worktree.state}" title={worktreeMarkTooltip(node.worktree)}
          >(<span class="wt-dot" aria-hidden="true"></span>worktree)</span
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

  .wt-mark {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .wt-dot {
    width: 6px;
    height: 6px;
    margin-right: var(--sp-2);
    border-radius: 50%;
    background: currentColor;
  }

  .wt-mark.changes .wt-dot {
    background: var(--indicator-changes);
  }

  .wt-mark.synced .wt-dot {
    background: var(--indicator-synced);
  }

  .wt-mark.missing {
    color: var(--status-delete);
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
