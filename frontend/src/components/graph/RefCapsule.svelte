<script lang="ts">
  import KindIcon from "$components/common/KindIcon.svelte";
  import { labelParts, type RefLabel } from "$lib/format";

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
    <!-- The HEAD colour stays on the local part: the prefix and `=` are in remote colours. -->
    <span class="capsule {label.kind} joined" class:holds={label.worktree}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="prefix" title={remote.text} oncontextmenu={menu(remote)} ondblclick={activate(remote)}
        >{label.prefix}</span
      ><!-- svelte-ignore a11y_no_static_element_interactions --><span
        class="eq"
        title={localTip}
        oncontextmenu={menu(parts.local)}
        ondblclick={activate(parts.local)}>=</span
      ><!-- svelte-ignore a11y_no_static_element_interactions --><span
        class="branch"
        title={localTip}
        oncontextmenu={menu(parts.local)}
        ondblclick={activate(parts.local)}>{@render held()}{text}</span
      >
    </span>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span
      class="capsule {label.kind}"
      class:holds={label.worktree}
      title={tooltip}
      oncontextmenu={menu(label)}
      ondblclick={activate(label)}
    >
      {@render held()}{text}
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
    border: 1px solid;
    border-radius: var(--r-md);
    font-family: var(--font-mono);
    font-size: var(--fs-small);
    line-height: 14px;
  }

  /* The ahead/behind counter, flush left of the branch label: two plain blocks, one group. */
  .group {
    display: inline-flex;
    flex: none;
  }

  .counter {
    display: inline-flex;
    align-items: center;
    height: 16px;
    padding: 0 var(--sp-2);
    white-space: nowrap;
    color: var(--badge-divergence-fg);
    background: var(--badge-divergence-bg);
    border: 1px solid var(--badge-divergence-fg);
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

  .head {
    color: var(--badge-head-fg);
    background: var(--badge-head-bg);
    border-color: var(--badge-head-bg);
  }

  .local {
    color: var(--badge-branch-fg);
    background: var(--badge-branch-bg);
    border-color: var(--badge-branch-fg);
  }

  .remote {
    color: var(--badge-remote-fg);
    background: var(--badge-remote-bg);
    border-color: var(--field-border);
  }

  .tag {
    color: var(--badge-tag-fg);
    background: var(--badge-tag-bg);
    border-color: var(--badge-tag-fg);
  }

  .stash {
    color: var(--status-stash);
    background: var(--c-stash-bg);
    border-color: var(--status-stash);
  }

  /* `origin=feature/x`: the remotes in the remote colours, the branch in its own. */
  .joined {
    padding: 0;
  }

  .prefix,
  .eq {
    flex: none;
    color: var(--badge-remote-fg);
    background: var(--badge-remote-bg);
  }

  .prefix {
    padding-left: var(--sp-3);
  }

  .eq {
    padding: 0 var(--sp-1);
  }

  .branch {
    display: inline-flex;
    align-items: center;
    padding: 0 var(--sp-3) 0 var(--sp-2);
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
