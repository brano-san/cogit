<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import { refNameField } from "$lib/names";
  import { addRemoteProblem } from "$lib/ref-group-menus";

  /** Add Remote… (F-941): `git remote add <name> <url>`, then a fetch of it if ticked. */
  interface Props {
    /** The remotes there are, so a taken name is caught before git refuses it. */
    taken: readonly string[];
    onadd: (name: string, url: string, fetch: boolean) => void;
    onclose: () => void;
  }

  let { taken, onadd, onclose }: Props = $props();

  // The first remote of a repository is usually origin.
  // svelte-ignore state_referenced_locally
  let name = $state(taken.includes("origin") ? "" : "origin");
  let url = $state("");
  let fetch = $state(true);

  const problem = $derived(addRemoteProblem(name, url, taken));

  function submit() {
    if (problem === null) onadd(name.trim(), url.trim(), fetch);
  }
</script>

<Dialog title="Add Remote" {onclose} onconfirm={submit} dirty={url.trim() !== ""} width="min(560px, 92vw)">
  <div class="form">
    <label class="field">
      <span class="caption">Name</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="text" bind:value={name} use:refNameField autofocus spellcheck="false" aria-label="Name" />
    </label>
    <label class="field">
      <span class="caption">URL or path</span>
      <input
        type="text"
        class="mono"
        bind:value={url}
        spellcheck="false"
        placeholder="https://example.com/team/project.git"
        aria-label="URL or path"
      />
    </label>
    <Checkbox bind:checked={fetch} label="Fetch after adding" />
  </div>

  {#snippet footer()}
    {#if problem}<span class="problem">{problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>Add</button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .caption {
    color: var(--text-secondary);
  }

  .problem {
    color: var(--status-delete);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
