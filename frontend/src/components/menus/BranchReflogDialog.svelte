<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { branchReflog, type ReflogEntry, type RepoId } from "$lib/ipc";
  import { shortDate, shortOid } from "$lib/format";

  interface Props {
    repo: RepoId;
    branch: string;
    onrestore: (entry: ReflogEntry) => void;
    onclose: () => void;
  }

  let { repo, branch, onrestore, onclose }: Props = $props();

  let entries = $state.raw<ReflogEntry[] | null>(null);
  let failure = $state<string | null>(null);
  let chosen = $state<number | null>(null);

  $effect(() => {
    branchReflog(repo, branch)
      .then((found) => (entries = found))
      .catch((err) => (failure = String(err?.message ?? err)));
  });

  const selected = $derived(chosen === null ? null : (entries?.[chosen] ?? null));

  function submit() {
    if (selected && chosen !== 0) onrestore(selected);
  }
</script>

<Dialog title="Reflog of {branch}" {onclose} onconfirm={submit} width="min(720px, 92vw)">
  {#if failure}
    <p class="why">{failure}</p>
  {:else if entries === null}
    <p class="why">Reading the reflog…</p>
  {:else if entries.length === 0}
    <p class="why">This branch has no reflog.</p>
  {:else}
    <ul class="list" role="listbox" aria-label="Reflog entries">
      {#each entries as entry, index (entry.selector)}
        <li
          role="option"
          aria-selected={chosen === index}
          tabindex="0"
          onkeydown={(event) => {
            if (event.key === "Enter") {
              chosen = index;
              submit();
            }
          }}
          class:chosen={chosen === index}
          onclick={() => (chosen = index)}
          ondblclick={() => {
            chosen = index;
            submit();
          }}
        >
          <span class="mono sel">{entry.selector}</span>
          <span class="mono">{shortOid(entry.oid)}</span>
          <span class="action">{entry.action}</span>
          <span class="msg truncate" title={entry.message}>{entry.message}</span>
          <span class="date">{shortDate(entry.timestamp, 0)}</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#snippet footer()}
    {#if chosen === 0}<span class="why">the branch is already here</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Close</button>
    <button type="button" class="btn primary" disabled={!selected || chosen === 0} onclick={submit}>
      Restore Branch Here
    </button>
  {/snippet}
</Dialog>

<style>
  .list {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 50vh;
    overflow: auto;
    font-size: var(--fs-dense);
  }

  li {
    display: flex;
    gap: var(--sp-4);
    padding: var(--sp-2) var(--sp-3);
    cursor: default;
  }

  li.chosen {
    background: var(--surface-selected, var(--surface-raised));
  }

  .sel {
    flex: 0 0 7em;
  }

  .action {
    flex: 0 0 auto;
    color: var(--text-secondary);
  }

  .msg {
    flex: 1 1 auto;
    min-width: 0;
  }

  .date,
  .why {
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
