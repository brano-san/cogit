<script lang="ts">
  import type { Snippet } from "svelte";

  /** A titled work surface. Every panel in the grid uses this shell. */
  interface Props {
    title: string;
    /** Optional count shown next to the title, e.g. "Files (23)". Zero is not worth the
        parentheses: three panels showing "(0)" is three ways of saying nothing is here. */
    count?: number;
    /** Controls placed at the right of the header, such as a filter field. */
    actions?: Snippet;
    children?: Snippet;
    /** Shown instead of `children`; set it only in the state where the body is empty. */
    empty?: string;
    /** Something changed on disk and this panel has not caught up yet. */
    stale?: boolean;
  }

  let { title, count, actions, children, empty, stale = false }: Props = $props();
</script>

<section class="panel">
  <header class="panel-header">
    <h2 class="panel-title">
      {title}{#if count}&nbsp;({count}){/if}
    </h2>
    {#if stale}
      <span class="stale" title="Something changed on disk; this is being reloaded">•</span>
    {/if}
    {#if actions}
      <div class="panel-actions">{@render actions()}</div>
    {/if}
  </header>

  <div class="panel-body">
    {#if empty}
      <p class="panel-empty">{empty}</p>
    {:else if children}
      {@render children()}
    {/if}
  </div>
</section>

<style>
  .stale {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-size: 14px;
    line-height: 1;
  }

  /* An outline of its own, so a panel is a block even where no splitter borders it. */
  .panel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--surface-panel);
    border: 1px solid var(--divider);
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    height: var(--h-panel-hdr);
    flex: 0 0 var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
  }

  .panel-title {
    margin: 0;
    font-size: var(--fs-header);
    font-weight: 600;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .panel-actions {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .panel-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .panel-empty {
    margin: 0;
    padding: var(--sp-7) var(--sp-5);
    text-align: center;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
