<script lang="ts">
  import type { Activity } from "$lib/operations";

  /** The one activity indicator in the app (doc/05-ui-layout.md section 3.7). */
  interface Props {
    repository?: string;
    branch?: string;
    /** Relationship to upstream, e.g. "= origin". */
    upstream?: string;
    ahead?: number;
    behind?: number;
    summary?: string;
    encoding?: string;
    lineEnding?: string;
    /** A file is showing in the diff panel, so the encoding pair describes something. */
    fileOpen?: boolean;
    activity: Activity;
    /** Commands that failed or printed something on stderr; opens the Output panel. */
    problems?: number;
    onproblems?: () => void;
    /** No working git: writes will fail. Opens the dialog that finds one. */
    gitMissing?: boolean;
    ongitmissing?: () => void;
    /** Present while a fetch, pull or push runs: stops its git (R-506). */
    oncancel?: () => void;
  }

  let {
    repository,
    branch,
    upstream,
    ahead = 0,
    behind = 0,
    summary,
    encoding = "UTF-8",
    lineEnding = "LF",
    fileOpen = false,
    activity,
    problems = 0,
    onproblems,
    gitMissing = false,
    ongitmissing,
    oncancel,
  }: Props = $props();
</script>

<footer class="status-bar">
  <button
    type="button"
    class="foot-button output"
    class:problems={problems > 0}
    onclick={() => onproblems?.()}
    title="Show Output"
    aria-label="Show Output"
  >
    {#if problems > 0}
      <svg class="warn" viewBox="0 0 16 16" aria-hidden="true"
        ><path
          fill="currentColor"
          fill-rule="evenodd"
          d="M6.92 2.4a1.25 1.25 0 0 1 2.16 0l5.77 10a1.25 1.25 0 0 1-1.08 1.87H2.23a1.25 1.25 0 0 1-1.08-1.87ZM7.25 5.75v4.5h1.5v-4.5ZM8 11.1a.9.9 0 1 0 0 1.8.9.9 0 1 0 0-1.8Z"
        /></svg
      >
      <span class="tabular">{problems}</span>
    {:else}
      <svg class="log" viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"
        ><path d="M2 3.5h1M2 8h1M2 12.5h1M5.5 3.5h8.5M5.5 8h8.5M5.5 12.5h6" /></svg
      >
    {/if}
  </button>
  <span class="divider" aria-hidden="true"></span>

  {#if gitMissing}
    <button type="button" class="foot-button problems" onclick={() => ongitmissing?.()} title="Git was not found: Cogit cannot change repositories. Click to fix.">
      Git not found
    </button>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  {#if repository}
    <span class="item shrink-last">
      <svg class="folder" viewBox="0 0 16 16" aria-hidden="true"
        ><path
          fill="currentColor"
          d="M1.5 3.5c0-.69.56-1.25 1.25-1.25h3.04c.4 0 .78.19 1.01.51l.79 1.09h5.66c.69 0 1.25.56 1.25 1.25v7.15c0 .69-.56 1.25-1.25 1.25H2.75c-.69 0-1.25-.56-1.25-1.25V3.5Z"
        /></svg
      >
      <span class="truncate">{repository}</span>
    </span>
    {#if branch || summary}<span class="divider" aria-hidden="true"></span>{/if}
  {/if}

  <!-- No tooltip on the repository, the branch or the summary: it only repeated them; the
       ahead/behind one spells out the arrows. Names give up the width before the status on the right does, the upstream and the
       summary first: they are the secondary text (R-243). -->
  {#if branch}
    <span class="item shrink-last">
      <span class="glyph" aria-hidden="true">⎇</span>
      <span class="branch truncate shrink-last">{branch}</span>
      {#if upstream}<span class="muted truncate shrink-first">({upstream})</span>{/if}
    </span>
    <span class="divider" aria-hidden="true"></span>
    <span class="item tabular" title="{ahead} ahead, {behind} behind">↑{ahead} ↓{behind}</span>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  {#if summary}
    <span class="item shrink-first"><span class="truncate">{summary}</span></span>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  <span class="spacer"></span>

  <span class="item activity {activity.tone}" role="status" title={activity.label}>
    {#if activity.busy}
      <!-- Determinate while the step says how far it is, else a sweep: an operation is under way. -->
      <span class="bar" class:sweep={activity.fraction === null} aria-hidden="true">
        <span class="fill" style:width={activity.fraction === null ? undefined : `${activity.fraction * 100}%`}></span>
      </span>
    {/if}
    <span class="truncate">{activity.label}</span>
  </span>
  {#if oncancel}
    <button type="button" class="foot-button cancel" onclick={() => oncancel()} title="Stop the fetch, pull or push that is running">
      Cancel
    </button>
  {/if}

  {#if fileOpen}
    <span class="divider" aria-hidden="true"></span>
    <span class="item muted encoding">{encoding} • {lineEnding}</span>
  {/if}
</footer>

<style>
  /* Content centred in a box centred on the bar (#18): an icon, not the ⚠ of whichever
     fallback font had one, and a line box as tall as the text, so no font lifts it. */
  .foot-button {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    flex: none;
    height: 18px;
    padding: 0 var(--sp-3);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    line-height: 1;
    cursor: default;
  }

  .foot-button:hover {
    color: var(--status-delete);
  }

  .output:not(.problems) {
    color: var(--text-secondary);
  }

  .output:not(.problems):hover {
    color: var(--text-primary);
  }

  .problems {
    color: var(--status-modify);
  }

  .warn {
    width: 12px;
    height: 12px;
  }

  .log {
    width: 16px;
    height: 16px;
  }

  .cancel {
    border-color: var(--divider);
    color: var(--text-primary);
  }

  .status-bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-statusbar);
    flex: 0 0 var(--h-statusbar);
    padding: 0 var(--sp-5);
    background: var(--chrome-bg);
    border-top: 1px solid var(--chrome-border);
    font-size: var(--fs-status);
    line-height: 16px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
  }

  .item {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .divider,
  .folder,
  .glyph {
    flex: none;
  }

  .branch {
    color: var(--status-ref);
  }

  .folder {
    width: 12px;
    height: 12px;
    color: var(--status-ref);
  }

  .divider {
    width: 1px;
    height: 12px;
    background: var(--divider);
    margin-inline: var(--sp-2);
  }

  .spacer {
    flex: 1 1 auto;
  }

  /* Right-aligned but not shoving: the status is as wide as it needs and no wider, and
     it does not push the fixed blocks about as its text changes (R-134). */
  .activity {
    flex: 0 0 auto;
    min-width: 0;
    max-width: 320px;
    justify-content: flex-end;
    font-variant-numeric: tabular-nums;
  }

  /* Fixed content, fixed box: it must not shift when the status beside it changes. */
  .encoding {
    flex: 0 0 auto;
    min-width: 92px;
    justify-content: flex-end;
  }

  .activity.error {
    color: var(--status-delete);
  }

  .bar {
    position: relative;
    flex: 0 0 auto;
    width: 96px;
    height: 6px;
    /* The item's own gap left it touching the text: a bar needs air a glyph does not. */
    margin-right: var(--sp-3);
    overflow: hidden;
    border-radius: 3px;
    background: var(--divider);
  }

  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 3px;
    background: var(--status-ref);
    transition: width 150ms linear;
  }

  .sweep .fill {
    width: 35%;
    animation: sweep 1.1s ease-in-out infinite;
  }

  @keyframes sweep {
    from {
      left: -35%;
    }
    to {
      left: 100%;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sweep .fill {
      left: 0;
      width: 100%;
      opacity: 0.5;
      animation: none;
    }
  }
</style>
