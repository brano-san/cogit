<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Select from "$components/common/Select.svelte";
  import { initialUpstream, upstreamProblem } from "$lib/upstream";

  /** Set Upstream… of a local branch (F-130): the remote branch it tracks from now on. */
  interface Props {
    branch: string;
    current: string | null;
    /** Remote branches, e.g. `origin/main`. */
    choices: readonly string[];
    primary: string | null;
    onset: (upstream: string) => void;
    onclose: () => void;
  }

  let { branch, current, choices, primary, onset, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let choice = $state(initialUpstream(branch, current, choices, primary));
  const problem = $derived(upstreamProblem(choice, current, choices));

  function submit() {
    if (problem === null && choice !== null) onset(choice);
  }
</script>

<Dialog title="Set Upstream" {onclose} onconfirm={submit} width="min(520px, 92vw)">
  <div class="form">
    <p class="what">
      The branch <span class="mono">{branch}</span> tracks
      {#if current}<span class="mono">{current}</span>{:else}nothing yet{/if}.
    </p>
    <div class="field">
      <span class="caption">Remote branch</span>
      {#if choices.length > 0 && choice !== null}
        <Select
          value={choice}
          label="Remote branch"
          options={choices.map((name) => [name, name] as const)}
          onchange={(next) => (choice = next)}
        />
      {:else}
        <span class="mono">none</span>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    {#if problem}<span class="why">{problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>Set Upstream</button>
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

  .what {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .field {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-4);
  }

  .caption {
    color: var(--text-secondary);
  }

  .why {
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
