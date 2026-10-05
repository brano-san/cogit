<script lang="ts">
  import type { Snippet } from "svelte";
  import type { PanelView } from "$lib/repo-phase";
  import { StaleDot } from "$lib/staleness";

  /** A titled work surface. Every panel in the grid uses this shell.

      The header and the body read one field. "Branches (9)" over "Opening repository…"
      was two of them disagreeing, and no amount of care at the call site prevents that
      from coming back — so the panel decides both (doc/12-risks.md, R-119). */
  interface Props {
    title: string;
    /** What the repository is doing. Anything but `content` and the body is blank, with no count beside
        the title: there is nothing to have counted. The window says why once (StartScreen). */
    view?: PanelView;
    /** Optional count shown next to the title, e.g. "Files (23)". Zero is not worth the
        parentheses: three panels showing "(0)" is three ways of saying nothing is here. */
    count?: number;
    /** What the count counts, when the rows below are not simply that many. */
    countTitle?: string;
    actions?: Snippet;
    children?: Snippet;
    /** Something changed on disk and this panel has not caught up yet. */
    stale?: boolean;
    /** The panel the keyboard is talking to; its header says so (issue 15). */
    active?: boolean;
    /** The work surface of the graph and the diff is the editor tone, not the panel one. */
    surface?: "panel" | "editor";
    /** The count is still growing: an ellipsis after it, nothing in the body (R-301). */
    busy?: boolean;
  }

  let {
    title,
    view = "content",
    count,
    countTitle,
    actions,
    children,
    stale = false,
    active = false,
    busy = false,
    surface = "panel",
  }: Props = $props();

  const ready = $derived(view === "content");

  let dotShown = $state(false);
  const dot = new StaleDot((shown) => (dotShown = shown));
  $effect(() => dot.set(stale));
  $effect(() => () => dot.dispose());
</script>

<section class="panel" class:editor={surface === "editor"} class:active aria-busy={view === "opening"}>
  <header class="panel-header" class:active>
    <h2 class="panel-title">
      {title}{#if ready && count}&nbsp;<span title={countTitle}>({count}{#if busy}<span title="Loading the rest">…</span>{/if})</span>{/if}
    </h2>
    {#if dotShown}
      <span class="stale" title="Something changed on disk; this is being reloaded">•</span>
    {/if}
    {#if actions}
      <div class="panel-actions">{@render actions()}</div>
    {/if}
  </header>

  <div class="panel-body">
    {#if ready && children}
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
    /* A selected row is the stronger blue only while its panel has the keyboard. */
    --state-selected: var(--bg-selected-inactive);
    border: 1px solid var(--divider);
    overflow: hidden;
  }

  .panel.active {
    --state-selected: var(--bg-selected);
  }

  .panel.editor {
    background: var(--surface-editor);
  }

  .panel-header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    height: var(--h-panel-hdr);
    flex: 0 0 var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    background: var(--surface-base);
    border-bottom: 1px solid var(--divider);
  }

  /* The panel that has the keyboard: a 2px line under its header, over the border. */
  .panel-header.active {
    box-shadow: inset 0 -2px 0 var(--panel-active-header);
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

  /* Narrow, a field in it gives way (its own `min-width` says how far); a button never
     shrinks below its label, so it is never the one cut. */
  .panel-actions {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 1 auto;
    min-width: 0;
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
</style>
