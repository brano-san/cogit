<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import { removeRows } from "$lib/file-dialogs";

  /** SmartGit's Remove (#40): off by default the files only stop being tracked and stay
      on disk (`git rm --cached`); with "Delete local files" they go from disk too. */
  interface Props {
    paths: readonly string[];
    onremove: (paths: string[], deleteLocal: boolean) => void;
    onclose: () => void;
  }

  let { paths, onremove, onclose }: Props = $props();

  const rows = $derived(removeRows(paths));
  // svelte-ignore state_referenced_locally
  let ticked = $state(new Set(paths));
  let deleteLocal = $state(false);

  const chosen = $derived(rows.filter((row) => ticked.has(row.path)).map((row) => row.path));

  function tick(path: string, on: boolean) {
    const next = new Set(ticked);
    if (on) next.add(path);
    else next.delete(path);
    ticked = next;
  }

  function submit() {
    if (chosen.length > 0) onremove(chosen, deleteLocal);
  }
</script>

<Dialog title="Remove" {onclose} onconfirm={submit} width="min(560px, 92vw)">
  <div class="body">
    <h3>Remove files from the repository</h3>
    <p class="hint">
      Select the files you want to remove from the repository or working tree (stopped from tracking).
    </p>
    <div class="table" role="table" aria-label="Files to remove">
      <div class="row head" role="row">
        <span class="tick" role="columnheader"></span>
        <span role="columnheader">Name</span>
        <span role="columnheader">Directory</span>
      </div>
      <div class="rows">
        {#each rows as row (row.path)}
          <div class="row" role="row">
            <span class="tick" role="cell">
              <Checkbox checked={ticked.has(row.path)} label="" onchange={(on) => tick(row.path, on)} />
            </span>
            <span class="mono truncate" role="cell" title={row.path}>{row.name}</span>
            <span class="mono truncate dir" role="cell" title={row.directory}>{row.directory}</span>
          </div>
        {/each}
      </div>
    </div>
    <Checkbox bind:checked={deleteLocal} label="Delete local files" />
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn" class:primary={!deleteLocal} class:warning={deleteLocal} disabled={chosen.length === 0} onclick={submit}>
      Remove
    </button>
  {/snippet}
</Dialog>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    font-size: var(--fs-dense);
  }

  h3 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .hint {
    margin: 0;
    color: var(--text-secondary);
  }

  .table {
    display: flex;
    flex-direction: column;
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
  }

  .rows {
    max-height: 16em;
    overflow-y: auto;
  }

  .row {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) minmax(0, 1.2fr);
    align-items: center;
    gap: var(--sp-3);
    min-height: 22px;
    padding: 0 var(--sp-3);
  }

  .row.head {
    color: var(--text-secondary);
    border-bottom: 1px solid var(--divider);
  }

  .dir {
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
