<script lang="ts">
  import type { Snippet } from "svelte";
  import type { PanelView } from "$lib/repo-phase";

  /** A titled work surface. Every panel in the grid uses this shell.

      The header and the body read one field. "Branches (9)" over "Opening repository…"
      was two of them disagreeing, and no amount of care at the call site prevents that
      from coming back — so the panel decides both (doc/12-risks.md, R-119). */
  interface Props {
    title: string;
    /** What the repository is doing. Anything but `content` and the body is the message
        below, with no count beside the title: there is nothing to have counted. */
    view?: PanelView;
    /** Optional count shown next to the title, e.g. "Files (23)". Zero is not worth the
        parentheses: three panels showing "(0)" is three ways of saying nothing is here. */
    count?: number;
    /** Controls placed at the right of the header, such as a filter field. */
    actions?: Snippet;
    children?: Snippet;
    /** Shown instead of `children` while the panel has its own reason to be empty. */
    empty?: string;
    /** Something changed on disk and this panel has not caught up yet. */
    stale?: boolean;
    /** The panel the keyboard is talking to; its header says so (issue 15). */
    active?: boolean;
  }

  let {
    title,
    view = "content",
    count,
    actions,
    children,
    empty,
    stale = false,
    active = false,
  }: Props = $props();

  const ready = $derived(view === "content");
  const message = $derived(
    ready ? empty : view === "opening" ? "Opening repository…" : "No repository open.",
  );
</script>

<section class="panel">
  <header class="panel-header" class:active>
    <h2 class="panel-title">
      {title}{#if ready && count}&nbsp;({count}){/if}
    </h2>
    {#if stale}
      <span class="stale" title="Something changed on disk; this is being reloaded">•</span>
    {/if}
    {#if actions}
      <div class="panel-actions">{@render actions()}</div>
    {/if}
  </header>

  <div class="panel-body">
    {#if message}
      <p class="panel-empty">{message}</p>
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

  .panel-header.active {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .panel-header.active .panel-title {
    color: var(--text-primary);
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
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .panel-body > :global(*) {
    flex: 1 1 auto;
    min-height: 0;
  }

  .panel-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0;
    padding: var(--sp-7) var(--sp-5);
    text-align: center;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
