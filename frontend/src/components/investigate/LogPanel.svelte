<script lang="ts">
  import {
    commitDetails,
    commitFiles,
    type CommitDetails,
    type FileEntry,
    type RepoId,
  } from "$lib/ipc";
  import { dateOf } from "$lib/investigate/blame";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";

  /** The Log perspective: the selected commit and every file it changed. */
  interface Props {
    repo: RepoId;
    session: InvestigateSession;
  }

  let { repo, session }: Props = $props();

  let details = $state.raw<CommitDetails | null>(null);
  let files = $state.raw<FileEntry[]>([]);
  let error = $state<string | null>(null);

  const LETTER: Partial<Record<FileEntry["status"], string>> = {
    added: "A",
    modified: "M",
    deleted: "D",
    renamed: "R",
    copied: "C",
  };

  $effect(() => {
    const rev = session.location.rev;
    details = null;
    files = [];
    error = null;
    if (!rev) return;
    let live = true;
    Promise.all([commitDetails(repo, rev), commitFiles(repo, rev)])
      .then(([found, changed]) => {
        if (!live) return;
        details = found;
        files = changed;
      })
      .catch((err: unknown) => {
        if (live) error = err instanceof Error ? err.message : String(err);
      });
    return () => {
      live = false;
    };
  });
</script>

<section class="panel">
  {#if session.location.rev === null}
    <p class="note">The working tree has no commit yet. Pick a commit in Navigation.</p>
  {:else if error}
    <p class="note error">{error}</p>
  {:else if !details}
    <p class="note">Loading…</p>
  {:else}
    <div class="commit">
      <div class="summary">{details.summary}</div>
      {#if details.body}<pre class="body">{details.body}</pre>{/if}
      <dl>
        <dt>Commit</dt>
        <dd class="mono">{details.oid}</dd>
        <dt>Author</dt>
        <dd>{details.author.name} &lt;{details.author.email}&gt;, {dateOf(details.author.timestamp)}</dd>
        {#if details.parents.length > 0}
          <dt>{details.parents.length > 1 ? "Parents" : "Parent"}</dt>
          <dd class="mono">{details.parents.map((parent) => parent.slice(0, 7)).join(", ")}</dd>
        {/if}
      </dl>
    </div>
    <header>Changed files <span class="muted tabular">{files.length}</span></header>
    <ul>
      {#each files as file (file.path)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
        <li
          class:investigated={file.path === session.location.path}
          title="Double-click to investigate this file at this commit"
          ondblclick={() =>
            void session.navigate({ path: file.path, rev: session.location.rev, line: null })}
        >
          <span class="status {file.status}">{LETTER[file.status] ?? "·"}</span>
          <span class="mono truncate">{file.path}</span>
          {#if file.oldPath}<span class="muted truncate">from {file.oldPath}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
    background: var(--surface-base);
    font-size: var(--fs-dense);
  }

  .note {
    margin: 0;
    padding: var(--sp-6);
    color: var(--text-secondary);
  }

  .error {
    color: var(--status-delete);
    white-space: pre-wrap;
  }

  .commit {
    padding: var(--sp-5) var(--sp-6);
    border-bottom: 1px solid var(--divider);
  }

  .summary {
    font-weight: 600;
  }

  .body {
    margin: var(--sp-3) 0 0;
    font-family: var(--font-ui);
    white-space: pre-wrap;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--sp-1) var(--sp-5);
    margin: var(--sp-4) 0 0;
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
  }

  header {
    padding: var(--sp-3) var(--sp-6);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-weight: 600;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-row);
    padding: 0 var(--sp-6);
    cursor: default;
  }

  li:hover {
    background: var(--state-hover);
  }

  li.investigated {
    background: var(--state-selected);
  }

  .status {
    flex: 0 0 2ch;
    font-family: var(--font-mono);
    color: var(--status-modify);
  }

  .status.added {
    color: var(--status-add);
  }

  .status.deleted {
    color: var(--status-delete);
  }
</style>
