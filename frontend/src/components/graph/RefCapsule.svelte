<script lang="ts">
  import KindIcon from "$components/common/KindIcon.svelte";
  import HereMark from "$components/graph/HereMark.svelte";
  import { labelParts, type RefLabel } from "$lib/format";
  import { labelDrag } from "$lib/drop-target";

  interface Props {
    label: RefLabel;
    /** Right-click: the menu of the ref under the pointer, not the row's (#39); in a joined
        label each part is a ref of its own. */
    onmenu?: (event: MouseEvent, ref: RefLabel) => void;
    /** Double click: that ref's own Check Out, not its commit's (item 40). */
    onactivate?: (ref: RefLabel) => void;
  }

  let { label, onmenu, onactivate }: Props = $props();

  const parts = $derived(labelParts(label));
  const tooltip = $derived(label.title ?? label.text);
  /** The joined label's tooltip without its upstream line: the local part's. */
  const localTip = $derived(tooltip.split("\n").filter((line) => line !== label.upstream).join("\n"));
  const text = $derived(label.upstream !== undefined ? (label.name ?? "") : label.text);

  function menu(ref: RefLabel) {
    return (event: MouseEvent) => onmenu?.(event, ref);
  }

  function activate(ref: RefLabel) {
    return (event: MouseEvent) => {
      if (!onactivate) return;
      event.stopPropagation();
      onactivate(ref);
    };
  }
</script>

{#snippet held()}
  {#if label.worktree}
    <span class="held"><KindIcon kind="worktree" title={tooltip} /></span>
  {/if}
{/snippet}

{#snippet capsule()}
  {#if parts.remote}
    {@const remote = parts.remote}
    <!-- The branch colour stays on the local part: the prefix in remote colours, `=` olive. -->
    <span class="capsule {label.kind} joined" class:holds={label.worktree}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="prefix" title={remote.text} data-ref={labelDrag(remote)} oncontextmenu={menu(remote)} ondblclick={activate(remote)}
        >{label.prefix}</span
      ><!-- svelte-ignore a11y_no_static_element_interactions --><span
        class="eq"
        title={localTip}
        data-ref={labelDrag(parts.local)}
        oncontextmenu={menu(parts.local)}
        ondblclick={activate(parts.local)}>=</span
      ><!-- svelte-ignore a11y_no_static_element_interactions --><span
        class="branch"
        title={localTip}
        data-ref={labelDrag(parts.local)}
        oncontextmenu={menu(parts.local)}
        ondblclick={activate(parts.local)}>{@render held()}{#if label.kind === "head"}<HereMark />{/if}{text}</span
      >
    </span>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span
      class="capsule {label.kind}"
      class:holds={label.worktree}
      title={tooltip}
      data-ref={labelDrag(label)}
      oncontextmenu={menu(label)}
      ondblclick={activate(label)}
    >
      {@render held()}{#if label.kind === "head"}<HereMark />{/if}{text}
    </span>
  {/if}
{/snippet}

{#if label.divergence}
  <span class="group">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="counter" title={label.divergenceTitle} oncontextmenu={menu(parts.local)}>{label.divergence}</span
    >{@render capsule()}
  </span>
{:else}
  {@render capsule()}
{/if}

<style>
  .capsule {
    display: inline-flex;
    align-items: center;
    flex: none;
    height: 16px;
    padding: 0 var(--sp-3);
    white-space: nowrap;
    /* One outline for every label, as SmartGit; the kind is the fill. */
    border: 1px solid var(--badge-border);
    border-radius: var(--r-md);
    font-family: var(--font-mono);
    font-size: var(--fs-small);
    line-height: 14px;
  }

  /* The ahead/behind counter, flush left of the branch label: two plain blocks inside the
     labels' one outline, which the label's left edge closes. */
  .group {
    display: inline-flex;
    flex: none;
  }

  .counter {
    display: inline-flex;
    align-items: center;
    height: 16px;
    padding: 0 var(--sp-3);
    white-space: nowrap;
    color: var(--badge-divergence-fg);
    background: var(--badge-divergence-bg);
    border: 1px solid var(--badge-border);
    border-right: 0;
    border-radius: var(--r-md) 0 0 var(--r-md);
    font-family: var(--font-mono);
    font-size: var(--fs-small);
    line-height: 14px;
  }

  .group .capsule {
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
  }

  /* HEAD is a local branch like any other; the triangle says where HEAD is (SmartGit). */
  .head,
  .local {
    color: var(--badge-branch-fg);
    background: var(--badge-branch-bg);
  }

  .remote {
    color: var(--badge-remote-fg);
    background: var(--badge-remote-bg);
  }

  .tag {
    color: var(--badge-tag-fg);
    background: var(--badge-tag-bg);
  }

  .stash {
    color: var(--status-stash);
    background: var(--c-stash-bg);
    border-color: var(--status-stash);
  }

  /* `origin=feature/x`: the remotes in the remote colours, the branch in its own. */
  .joined {
    padding: 0;
    overflow: hidden;
  }

  .prefix,
  .eq {
    flex: none;
    color: var(--badge-remote-fg);
    background: var(--badge-remote-bg);
  }

  .eq {
    color: var(--badge-equal-fg);
    background: var(--badge-equal-bg);
  }

  /* Every part padded alike, a hairline of the outline between them. */
  .prefix,
  .eq,
  .branch {
    display: inline-flex;
    align-items: center;
    align-self: stretch;
    padding: 0 var(--sp-3);
  }

  .eq,
  .branch {
    border-left: 1px solid var(--badge-border);
  }

  /* The Worktrees panel's icon, in the label's own colour so it shows on a filled HEAD. */
  .held {
    --kind-icon: 10px;
    display: inline-flex;
    flex: none;
    margin-right: var(--sp-1);
  }

  .held :global(.kind.worktree) {
    color: inherit;
  }
</style>
