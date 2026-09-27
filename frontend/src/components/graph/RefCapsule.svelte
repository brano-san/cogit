<script lang="ts">
  import KindIcon from "$components/common/KindIcon.svelte";
  import type { RefLabel } from "$lib/format";

  interface Props {
    label: RefLabel;
    /** Right-click: the label's own menu, not the row's (#39). */
    onmenu?: (event: MouseEvent) => void;
    /** Double click: the label's own Check Out, not its commit's (item 40). */
    onactivate?: () => void;
  }

  let { label, onmenu, onactivate }: Props = $props();

  const tooltip = $derived(label.title ?? label.text);
  const prefix = $derived(label.remotes?.join(",") ?? "");
  const text = $derived(label.remotes ? (label.name ?? "") : label.text);
</script>

{#snippet held()}
  {#if label.worktree}
    <span class="held"><KindIcon kind="worktree" title={tooltip} /></span>
  {/if}
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  class="capsule {label.kind}"
  class:joined={label.remotes}
  class:holds={label.worktree}
  title={tooltip}
  oncontextmenu={onmenu}
  ondblclick={(event) => {
    if (!onactivate) return;
    event.stopPropagation();
    onactivate();
  }}
>
  {#if label.remotes}
    <span class="prefix">{prefix}</span><span class="eq">=</span><span class="branch"
      >{@render held()}{text}</span
    >
  {:else}
    {@render held()}{text}
  {/if}
</span>

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
    font-size: 10px;
    line-height: 14px;
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
