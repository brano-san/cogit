<script lang="ts">
  import type { RefLabel } from "$lib/format";

  interface Props {
    label: RefLabel;
  }

  let { label }: Props = $props();
</script>

<span class="capsule {label.kind}" class:joined={label.remotes} title={label.title ?? label.text}>
  {#if label.remotes}
    <span class="prefix">{label.remotes.join(",")}</span><span class="eq">=</span><span
      class="branch">{label.name}</span
    >
  {:else}
    {label.text}
  {/if}
</span>

<style>
  .capsule {
    flex: 0 0 auto;
    height: 16px;
    padding: 0 var(--sp-3);
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
    display: inline-flex;
    padding: 0;
    overflow: hidden;
  }

  .prefix,
  .eq {
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
    padding: 0 var(--sp-3) 0 var(--sp-2);
  }
</style>
