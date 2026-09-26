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
    oncancel,
  }: Props = $props();
</script>

<footer class="status-bar">
  {#if problems > 0}
    <button type="button" class="problems" onclick={() => onproblems?.()} title="Show Output">
      ⚠ {problems}
    </button>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  {#if repository}
    <span class="item shrink-last" title={repository}>
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

  <!-- Names give up the width before the status on the right does, the upstream and the
       summary first: they are the secondary text (R-243). -->
  {#if branch}
    <span class="item shrink-last" title={upstream ? `${branch} (${upstream})` : branch}>
      <span class="glyph" aria-hidden="true">⎇</span>
      <span class="branch truncate shrink-last">{branch}</span>
      {#if upstream}<span class="muted truncate shrink-first">({upstream})</span>{/if}
    </span>
    <span class="divider" aria-hidden="true"></span>
    <span class="item tabular" title="{ahead} ahead, {behind} behind">↑{ahead} ↓{behind}</span>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  {#if summary}
    <span class="item shrink-first" title={summary}><span class="truncate">{summary}</span></span>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  <span class="spacer"></span>

  <span class="item activity {activity.tone}" role="status">
    {#if activity.busy}<span class="spinner" aria-hidden="true"></span>{/if}
    <span class="truncate">{activity.label}</span>
  </span>
  {#if oncancel}
    <button type="button" class="cancel" onclick={() => oncancel()} title="Stop the fetch, pull or push that is running">
      Cancel
    </button>
  {/if}

  {#if fileOpen}
    <span class="divider" aria-hidden="true"></span>
    <span class="item muted encoding">{encoding} • {lineEnding}</span>
  {/if}
</footer>

<style>
  .problems {
    height: 16px;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--status-modify);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  .problems:hover {
    color: var(--status-delete);
  }

  .cancel {
    flex: none;
    height: 16px;
    padding: 0 var(--sp-3);
    background: none;
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  .cancel:hover {
    color: var(--status-delete);
  }

  .status-bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-statusbar);
    flex: 0 0 var(--h-statusbar);
    padding: 0 var(--sp-5);
    background: var(--titlebar-bg);
    border-top: 1px solid var(--titlebar-border);
    font-size: var(--fs-status);
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
  }

  .item {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .problems,
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

  .spinner {
    flex: 0 0 auto;
    width: 10px;
    height: 10px;
    border: 1.5px solid var(--divider);
    border-top-color: var(--status-ref);
    border-radius: 50%;
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
</style>
