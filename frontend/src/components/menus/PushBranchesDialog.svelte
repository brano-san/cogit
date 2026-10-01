<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Select from "$components/common/Select.svelte";
  import { pushBranchesTitle, startRemote, type PushBranch } from "$lib/push-branches";

  /** Push To for several local branches at once: one remote, each branch to its own
      tracked or same-named branch there. */
  interface Props {
    branches: readonly PushBranch[];
    remotes: readonly string[];
    primary: string | null;
    onpush: (remote: string, track: boolean) => void;
    onclose: () => void;
  }

  let { branches, remotes, primary, onpush, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let remote = $state(startRemote(branches, remotes, primary) ?? "");
  let track = $state(true);

  const problem = $derived(remote === "" ? "This repository has no remote." : null);
  const untracked = $derived(branches.some((branch) => branch.upstream === null));

  function submit() {
    if (problem === null) onpush(remote, track);
  }
</script>

<Dialog title="Push To" {onclose} onconfirm={submit} width="min(560px, 92vw)">
  <div class="form">
    <div class="heading">
      <h3>{pushBranchesTitle(branches.length, remote)}</h3>
      <p class="hint">Each branch goes to its tracked branch, or to a branch of the same name.</p>
    </div>

    <div class="field">
      <span class="caption">Remote</span>
      {#if remotes.length > 1}
        <Select value={remote} label="Remote" options={remotes.map((name) => [name, name] as const)} onchange={(next) => (remote = next)} />
      {:else}
        <span class="mono">{remote || "none"}</span>
      {/if}
    </div>

    <ul class="items mono">
      {#each branches as branch (branch.name)}<li>{branch.name}</li>{/each}
    </ul>

    {#if untracked}
      <div class="field">
        <Checkbox bind:checked={track} label="Set upstream" />
        <span class="hint">Branches that track nothing track the pushed branch from then on.</span>
      </div>
    {/if}
  </div>

  {#snippet footer()}
    {#if problem}<span class="problem">{problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>Push</button>
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

  .heading {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  h3 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .field {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-4);
  }

  .caption,
  .hint {
    color: var(--text-secondary);
  }

  .hint {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .items {
    max-height: 12em;
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    overflow-y: auto;
    list-style: none;
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    overflow-wrap: anywhere;
    user-select: text;
  }

  .problem {
    color: var(--status-delete);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
