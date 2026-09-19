<script lang="ts">
  /** Bottom status bar (doc/05-ui-layout.md section 3.7). */
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
    /** Application version, proving the IPC round-trip works. */
    version?: string;
    status?: string;
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
    version,
    status = "Ready",
  }: Props = $props();
</script>

<footer class="status-bar">
  {#if repository}
    <span class="item"><span aria-hidden="true">🗁</span> {repository}</span>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  {#if branch}
    <span class="item">
      <span aria-hidden="true">⎇</span>
      <span class="branch">{branch}</span>
      {#if upstream}<span class="muted">({upstream})</span>{/if}
    </span>
    <span class="divider" aria-hidden="true"></span>
    <span class="item tabular" title="{ahead} ahead, {behind} behind">↑{ahead} ↓{behind}</span>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  {#if summary}
    <span class="item">{summary}</span>
    <span class="divider" aria-hidden="true"></span>
  {/if}

  <span class="spacer"></span>

  <span class="item muted">{encoding} • {lineEnding}</span>
  <span class="divider" aria-hidden="true"></span>
  {#if version}
    <span class="item muted tabular" title="Cogit version, read over IPC">v{version}</span>
    <span class="divider" aria-hidden="true"></span>
  {/if}
  <span class="item">{status}</span>
</footer>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-statusbar);
    flex: 0 0 var(--h-statusbar);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    border-top: 1px solid var(--divider);
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

  .branch {
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
</style>
