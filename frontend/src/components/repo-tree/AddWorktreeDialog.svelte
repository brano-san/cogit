<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Radio from "$components/common/Radio.svelte";
  import Select from "$components/common/Select.svelte";
  import { addProblem, type BranchChoice } from "$lib/worktree-list";

  /** Folder, branch — new or existing — and where a new branch starts (R-184). */
  interface Props {
    choices: readonly BranchChoice[];
    /** Where a new branch starts unless changed: the selected commit, or HEAD. */
    base: string;
    onbrowse: () => Promise<string | null>;
    onadd: (request: { folder: string; branch: string; create: boolean; base: string | null }) => void;
    onclose: () => void;
  }

  let { choices, base: startAt, onbrowse, onadd, onclose }: Props = $props();

  let folder = $state("");
  let create = $state(true);
  let name = $state("");
  // svelte-ignore state_referenced_locally
  let existing = $state(choices.find((choice) => choice.heldBy === null)?.name ?? choices[0]?.name ?? "");
  // svelte-ignore state_referenced_locally
  let base = $state(startAt);
  let field: HTMLInputElement | undefined = $state();

  const branch = $derived(create ? name : existing);
  const problem = $derived(addProblem({ folder, create, branch, choices }));

  function submit() {
    if (problem !== null) return;
    onadd({
      folder: folder.trim(),
      branch: branch.trim(),
      create,
      base: create && base.trim() !== "" ? base.trim() : null,
    });
  }

  async function browse() {
    const picked = await onbrowse();
    if (picked) folder = picked;
  }

  $effect(() => {
    field?.focus();
  });
</script>

<Dialog title="Add Worktree" {onclose} onconfirm={submit} width="min(520px, 92vw)">
  <div class="form">
    <label class="field">
      <span>Folder</span>
      <span class="with-button">
        <input bind:this={field} type="text" bind:value={folder} placeholder="Where the new checkout goes" />
        <button type="button" class="btn" onclick={() => void browse()}>Browse…</button>
      </span>
    </label>

    <fieldset>
      <legend>Branch</legend>
      <Radio name="worktree-branch" checked={create} onchange={() => (create = true)} label="New branch" />
      {#if create}
        <div class="nested">
          <label class="field">
            <span>Name</span>
            <input type="text" bind:value={name} placeholder="feature/…" />
          </label>
          <label class="field">
            <span>Base commit</span>
            <input type="text" class="mono" bind:value={base} placeholder="HEAD" />
          </label>
        </div>
      {/if}

      <Radio name="worktree-branch" checked={!create} onchange={() => (create = false)} label="Existing branch" />
      {#if !create}
        <div class="nested">
          <Select
            value={existing}
            label="Existing branch"
            options={choices.map((choice) => [
              choice.name,
              choice.heldBy ? `${choice.name} — checked out in ${choice.heldBy}` : choice.name,
            ] as const)}
            onchange={(next) => (existing = next)}
          />
          <p class="hint">A branch lives in one worktree at a time; one already checked out elsewhere cannot be picked.</p>
        </div>
      {/if}
    </fieldset>
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
    padding: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .with-button {
    display: flex;
    gap: var(--sp-3);
  }

  .with-button input {
    flex: 1 1 auto;
    min-width: 0;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    padding: 0;
    margin-bottom: var(--sp-2);
  }

  .nested {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-left: var(--sp-7);
  }

  .hint {
    margin: 0;
    color: var(--text-secondary);
  }

  .problem {
    color: var(--status-modify);
    font-size: var(--fs-dense);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
