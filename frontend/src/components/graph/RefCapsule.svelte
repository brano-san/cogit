<script lang="ts">
  import KindIcon from "$components/common/KindIcon.svelte";
  import type { RefLabel } from "$lib/format";
  import { REF_LABEL_MAX, middleCut } from "$lib/truncate";

  interface Props {
    label: RefLabel;
    /** Right-click: the label's own menu, not the row's (#39). */
    onmenu?: (event: MouseEvent) => void;
  }

  let { label, onmenu }: Props = $props();

  const tooltip = $derived(label.title ?? label.text);
  const prefix = $derived(label.remotes?.join(",") ?? "");
  /** Middle-cut (#5), once: the remotes stay whole, the branch name gives up its middle, in
      CSS, at the width the row leaves it and never past `REF_LABEL_MAX` characters. */
  const text = $derived(middleCut(label.remotes ? (label.name ?? "") : label.text));
  /** Characters a squeezed label keeps (#12): the tail, an ellipsis and the remotes. In `ch`
      of the label's own monospace font, so the floor is exact. */
  const floor = $derived((label.remotes ? prefix.length + 1 : 0) + text.tail.length + (text.lead ? 1 : 0));
</script>

{#snippet held()}
  {#if label.worktree}
    <span class="held"><KindIcon kind="worktree" title={tooltip} /></span>
  {/if}
{/snippet}

{#snippet cut()}
  <span class="lead">{text.lead}</span><span class="tail">{text.tail}</span>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  class="capsule {label.kind}"
  class:joined={label.remotes}
  class:holds={label.worktree}
  style:--floor="{floor}ch"
  style:--cap="{REF_LABEL_MAX}ch"
  title={tooltip}
  oncontextmenu={onmenu}
>
  {#if label.remotes}
    <span class="prefix">{prefix}</span><span class="eq">=</span><span class="branch"
      >{@render held()}{@render cut()}</span
    >
  {:else}
    {@render held()}{@render cut()}
  {/if}
</span>

<style>
  /* Gives way before the subject does (#12), cut in the middle: the lead shrinks behind an
     ellipsis and the tail, which tells branches apart, stays. */
  .capsule {
    --chrome: calc(2 * var(--sp-3) + 2px);
    display: inline-flex;
    align-items: center;
    flex: 0 100000 auto;
    min-width: calc(var(--floor) + var(--chrome) + var(--held, 0px));
    max-width: calc(var(--cap) + var(--chrome) + var(--held, 0px));
    height: 16px;
    padding: 0 var(--sp-3);
    overflow: hidden;
    border: 1px solid;
    border-radius: var(--r-md);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 14px;
  }

  .lead {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tail {
    flex: none;
  }

  .holds {
    --held: 12px;
  }

  .head {
    color: var(--c-bg-window);
    background: var(--status-ref);
    border-color: var(--status-ref);
  }

  .local {
    color: var(--status-ref);
    background: var(--c-branch-bg);
    border-color: var(--status-ref);
  }

  .remote {
    color: var(--text-secondary);
    background: transparent;
    border-color: var(--field-border);
  }

  .tag {
    color: var(--status-tag);
    background: var(--c-tag-bg);
    border-color: var(--status-tag);
  }

  .stash {
    color: var(--status-stash);
    background: var(--c-stash-bg);
    border-color: var(--status-stash);
  }

  /* `origin=feature/x`: the remotes in the remote colours, the branch in its own. */
  .joined {
    --chrome: calc(2 * var(--sp-3) + 2 * var(--sp-1) + var(--sp-2) + 2px);
    padding: 0;
  }

  .prefix,
  .eq {
    flex: none;
    color: var(--text-secondary);
    background: var(--surface-raised);
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
    min-width: 0;
    overflow: hidden;
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
