<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { statusBadge, statusTooltip } from "$lib/files";
  import type { FileEntry, WorktreeEntry } from "$lib/ipc";
  import { removalNeeds } from "$lib/worktree-list";

  /** Removing a worktree with work or submodules in it needs a second, separate yes: what
      is at stake, and a box for `--force` (R-184, R-434). */
  interface Props {
    entry: WorktreeEntry;
    /** `null` while the changes are still being read. */
    changes: readonly FileEntry[] | null;
    onremove: (force: boolean) => void;
    onclose: () => void;
  }

  let { entry, changes, onremove, onclose }: Props = $props();

  const SHOWN = 40;
  let force = $state(false);

  const needs = $derived(removalNeeds(entry, changes));
  const ready = $derived(changes !== null && (!needs.force || force));

  function submit() {
    if (ready) onremove(needs.force);
  }
</script>

<Dialog title="Remove Worktree" {onclose} onconfirm={submit} width="min(520px, 92vw)">
  <div class="body">
    <p>
      Remove the worktree <strong>{entry.name}</strong>
      {entry.branch ? `(${entry.branch})` : ""} and delete its folder?
    </p>
    <p class="path mono">{entry.path}</p>

    {#if changes === null}
      <p class="hint">Reading its changes…</p>
    {:else}
      {#if needs.dirty}
        <p class="warning">It has {changes.length} uncommitted change{changes.length === 1 ? "" : "s"}:</p>
        <ul class="changes">
          {#each changes.slice(0, SHOWN) as file (file.path)}
            <li>
              <span class="badge mono" title={statusTooltip(file.status)}>{statusBadge(file.status)}</span>
              <span class="mono truncate">{file.path}</span>
            </li>
          {/each}
          {#if changes.length > SHOWN}<li class="hint">and {changes.length - SHOWN} more</li>{/if}
        </ul>
      {:else}
        <p class="hint">It has no uncommitted changes.</p>
      {/if}
      {#if needs.submodules}
        <p class="warning">
          Submodules are checked out in it. Git removes it only with --force, which deletes their
          repositories too: commits not pushed from them are lost.
        </p>
      {/if}
      {#if needs.force}
        <label class="force">
          <input type="checkbox" bind:checked={force} />
          {needs.dirty
            ? "Remove it anyway (--force). The changes are put in a stash first; Undo applies it."
            : "Remove it anyway (--force), with its submodules."}
        </label>
      {/if}
    {/if}
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={!ready} onclick={submit}>
      {needs.force ? "Remove with --force" : "Remove"}
    </button>
  {/snippet}
</Dialog>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-5);
    font-size: var(--fs-dense);
  }

  p {
    margin: 0;
  }

  .path {
    color: var(--text-secondary);
    overflow-wrap: anywhere;
    user-select: text;
  }

  .warning {
    color: var(--status-modify);
  }

  .changes {
    max-height: 12em;
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    overflow-y: auto;
    list-style: none;
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
  }

  .changes li {
    display: flex;
    gap: var(--sp-3);
    min-width: 0;
  }

  .badge {
    flex: none;
    width: 12px;
    text-align: center;
  }

  .force {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }

  .hint {
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
