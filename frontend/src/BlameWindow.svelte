<script lang="ts">
  import { untrack } from "svelte";
  import BlameView from "$components/diff/BlameView.svelte";
  import Select from "$components/common/Select.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { installChildWindow, onMenuAction } from "$lib/child-window";
  import {
    BLAME_ROW_HEIGHT,
    age,
    changedSince,
    commitOfLine,
    cursorAfter,
    parseBlame,
    sinceOptions,
    viewOptions,
  } from "$lib/blame-window";
  import { shortOid } from "$lib/format";
  import { revealCommit, type CommitRow, type LineVersion } from "$lib/ipc";
  import { blameWindow as blame } from "$stores/blame-window.svelte";
  import { followSettings } from "$lib/settings-sync";
  import { settings } from "$stores/settings.svelte";

  const request = parseBlame(window.location.search);

  // No browser menu (R-127), Esc / Ctrl+W close, and the window's own menu bar reaches here.
  $effect(() => installChildWindow(window));
  // What Preferences changes in the main window reaches this one too (F-335).
  $effect(() => followSettings(() => void settings.reload()));
  $effect(() =>
    onMenuAction(window, (action) => {
      if (action === "refresh") void blame.refresh();
      else if (action === "toggle-history") blame.toggleHistory();
      else if (action === "show-commit") showCommit(blame.cursor);
    }),
  );

  // Once, on opening: what the loads read must not make this effect run them again.
  $effect(() =>
    untrack(() => {
      void settings.load();
      if (request) void blame.open(request);
    }),
  );

  const date = (row: CommitRow) => settings.formatDate(row.timestamp, row.tzOffsetMinutes);
  const highlighted = $derived(changedSince(blame.revisions, blame.since));
  const views = $derived(request ? viewOptions(blame.revisions, request.rev, date) : []);
  const sinces = $derived(sinceOptions(blame.revisions, date));
  const now = $derived.by(() => {
    void blame.history;
    return Date.now() / 1000;
  });

  let mainHeight = $state(0);

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "F5") {
      event.preventDefault();
      void blame.refresh();
      return;
    }
    const target = event.target as HTMLElement | null;
    if (target?.closest("select, input, textarea, button")) return;
    const page = Math.max(1, Math.floor(mainHeight / BLAME_ROW_HEIGHT) - 1);
    const next = cursorAfter(event.key, blame.cursor, blame.lines.length, page);
    if (next === null) return;
    event.preventDefault();
    blame.moveTo(next);
  }

  /** In the main window's graph, which is where a commit is looked at (F-062). */
  function showCommit(at: number) {
    const oid = commitOfLine(blame.lines, at);
    if (request && oid) void revealCommit(request.repo, oid);
  }

  /** Only a version under today's name can be opened: blame reads the file by its path. */
  function openVersion(version: LineVersion) {
    if (request && version.path === request.path) void blame.show(version.oid, version.line);
  }
</script>

<svelte:window {onkeydown} />

<TooltipLayer />

<div class="window">
  {#if !request}
    <p class="note">This window needs a file. Open it with Blame from the Diff panel or a file's menu.</p>
  {:else}
    <header class="bar">
      <span class="field">
        <span class="label">View Commit</span>
        <Select
          label="View Commit"
          value={blame.view}
          options={views}
          onchange={(oid) => void blame.show(oid)}
        />
      </span>
      <span class="field">
        <span class="label">Highlight: Changes Since</span>
        <Select
          label="Highlight: Changes Since"
          value={blame.since ?? ""}
          options={sinces}
          onchange={(oid) => (blame.since = oid === "" ? null : oid)}
        />
      </span>
      <span class="count tabular">{blame.lines.length} lines</span>
    </header>
    {#if blame.revisionsError}
      <p class="note error">The versions of this file could not be listed: {blame.revisionsError}</p>
    {/if}

    <div class="main" bind:clientHeight={mainHeight}>
      {#if blame.error}
        <p class="note error">{blame.error}</p>
      {:else if blame.loading && blame.lines.length === 0}
        <p class="note">Blaming {request.path}…</p>
      {:else}
        <BlameView
          lines={blame.lines}
          cursor={blame.cursor}
          {highlighted}
          onpick={(at) => blame.moveTo(at)}
          onopen={showCommit}
        />
      {/if}
    </div>

    {#if blame.historyShown}
      <section class="history" aria-label="History of current line">
        <div class="history-bar">
          <span>History of current line</span>
          <span class="muted tabular">line {blame.cursor + 1}</span>
          {#if blame.historyLoading}<span class="muted">reading…</span>{/if}
          <span class="spacer"></span>
          <button
            type="button"
            title="Hide (View ▸ History of Current Line)"
            onclick={() => blame.toggleHistory()}>✕</button
          >
        </div>
        {#if blame.historyError}
          <p class="note error">{blame.historyError}</p>
        {:else}
          <div class="history-rows" role="list">
            {#each blame.history as version, at (at)}
              <div
                class="history-row"
                role="listitem"
                title={version.path === request.path
                  ? `${version.summary} — double-click to view this version`
                  : `${version.summary} — the file was ${version.path} then`}
                ondblclick={() => openVersion(version)}
              >
                <span class="oid mono">{shortOid(version.oid)}</span>
                <span class="author truncate">{version.author}</span>
                <span class="age tabular">{age(version.timestamp, now)}</span>
                <span class="text mono truncate">{version.text}</span>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  {/if}
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface-base);
    color: var(--text-primary);
    font-family: var(--font-ui);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-6);
    flex: 0 0 auto;
    min-height: var(--h-toolbar);
    padding: var(--sp-3) var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 1 1 0;
    min-width: 0;
  }

  .field :global(.select) {
    flex: 1 1 auto;
  }

  .label {
    flex: 0 0 auto;
    color: var(--text-secondary);
  }

  .count {
    flex: 0 0 auto;
    color: var(--text-secondary);
  }

  .main {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
  }

  .history {
    display: flex;
    flex-direction: column;
    flex: 0 0 200px;
    min-height: 0;
    border-top: 1px solid var(--divider);
    background: var(--surface-panel);
  }

  .history-bar {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: 0 0 auto;
    height: var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-header);
  }

  .spacer {
    flex: 1 1 auto;
  }

  .muted {
    color: var(--text-secondary);
  }

  .history-rows {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .history-row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .history-row:hover {
    background: var(--state-hover);
  }

  .oid {
    flex: 0 0 auto;
    color: var(--status-ref);
  }

  .author {
    flex: 0 0 140px;
    min-width: 0;
  }

  .age {
    flex: 0 0 48px;
    color: var(--text-secondary);
    text-align: right;
  }

  .text {
    flex: 1 1 auto;
    min-width: 0;
    white-space: pre;
  }

  .note {
    margin: 0;
    padding: var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .error {
    color: var(--status-delete);
    user-select: text;
  }
</style>
