<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { STASH_MODES, stashNameProblem, type StashMode } from "$lib/stash-modes";
  import { stashDialog } from "$stores/stash-dialog.svelte";

  /** Stash All and Stash Selection, both driven by `stashDialog` (#29). */
  let name = $state("");
  let message = $state("");

  const pending = $derived(stashDialog.open);
  const problem = $derived(stashNameProblem(name));

  $effect(() => {
    if (!pending) return;
    name = "";
    message = "";
  });

  function create(mode: StashMode) {
    if (pending?.kind !== "create" || problem !== null) return;
    pending.settle({ mode, message: name.trim() });
  }

  function confirmSelection() {
    if (pending?.kind === "selection") pending.settle(message.trim());
  }
</script>

{#if pending?.kind === "create"}
  <Dialog
    title="Stash All"
    width="min(560px, 92vw)"
    onclose={() => stashDialog.cancel()}
    onconfirm={() => create("all")}
  >
    <label class="field">
      <span>Name</span>
      <input
        data-autofocus
        type="text"
        bind:value={name}
        aria-invalid={problem !== null}
        aria-describedby={problem ? "stash-name-problem" : undefined}
      />
      <!-- Under the field it belongs to, not in the footer beside the buttons (#29). -->
      {#if problem}<span class="problem" id="stash-name-problem">{problem}</span>{/if}
    </label>

    {#snippet footer()}
      <span class="grow"></span>
      {#each STASH_MODES as entry, index (entry.mode)}
        <button
          type="button"
          class="btn"
          class:primary={index === 0}
          disabled={problem !== null}
          title={entry.hint}
          onclick={() => create(entry.mode)}>{entry.label}</button
        >
      {/each}
      <button class="btn" type="button" onclick={() => stashDialog.cancel()}>Cancel</button>
    {/snippet}
  </Dialog>
{:else if pending?.kind === "selection"}
  <Dialog title="Stash Selection" onclose={() => stashDialog.cancel()} onconfirm={confirmSelection}>
    <p class="lead">
      {pending.paths.length === 1 ? "This file" : `These ${pending.paths.length} files`} will be
      stashed; the rest of the working tree stays as it is.
    </p>
    <ul class="paths">
      {#each pending.paths as path (path)}
        <li class="truncate" title={path}>{path}</li>
      {/each}
    </ul>
    <label class="field">
      <span>Message (optional)</span>
      <input data-autofocus type="text" bind:value={message} placeholder="WIP on the branch" />
    </label>

    {#snippet footer()}
      <span class="grow"></span>
      <button class="btn" type="button" onclick={() => stashDialog.cancel()}>Cancel</button>
      <button type="button" class="btn primary" onclick={confirmSelection}>OK</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .problem {
    color: var(--status-delete);
    font-size: 11px;
  }

  .lead {
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-dense);
  }

  .paths {
    max-height: 180px;
    margin: 0 0 var(--sp-5);
    padding: var(--sp-2) var(--sp-4);
    overflow-y: auto;
    list-style: none;
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .paths li {
    padding: 1px 0;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
