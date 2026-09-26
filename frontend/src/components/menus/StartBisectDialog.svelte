<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { startProblem } from "$lib/bisect";
  import { shortOid } from "$lib/format";
  import { lookUpWhenSettled, type Named } from "$lib/rev-lookup";

  /** SmartGit's Branch | Bisect | Start: the bad commit, and a good one now or later. */
  interface Props {
    bad: string;
    good: string;
    busy: boolean;
    /** The commit a field names, or null when it names none. */
    resolve: (rev: string) => Promise<{ oid: string; summary: string } | null>;
    onstart: (bad: string, good: string) => void;
    onclose: () => void;
  }

  let { bad: initialBad, good: initialGood, busy, resolve, onstart, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let bad = $state(initialBad);
  // svelte-ignore state_referenced_locally
  let good = $state(initialGood);
  let field: HTMLInputElement | undefined = $state();

  type Found = Named<{ oid: string; summary: string }>;
  let badCommit = $state.raw<Found>(null);
  let goodCommit = $state.raw<Found>(null);

  /** Typing is not a question per key: the field is read once it rests. */
  const SETTLE_MS = 200;

  $effect(() => lookUpWhenSettled(bad, resolve, (found) => (badCommit = found), SETTLE_MS));
  $effect(() => lookUpWhenSettled(good, resolve, (found) => (goodCommit = found), SETTLE_MS));

  const unknown = (found: Found, rev: string) =>
    found !== null && found.rev === rev.trim() && found.commit === null;
  const problem = $derived(
    startProblem(bad, good) ??
      (unknown(badCommit, bad) ? `No commit is called ${bad.trim()}.` : null) ??
      (unknown(goodCommit, good) ? `No commit is called ${good.trim()}.` : null) ??
      (badCommit?.commit && goodCommit?.commit && badCommit.commit.oid === goodCommit.commit.oid
        ? "The good and the bad commit are the same commit."
        : null),
  );

  function submit() {
    if (problem !== null || busy) return;
    onstart(bad, good);
  }

  $effect(() => {
    field?.focus();
  });
</script>

{#snippet named(found: Found, rev: string)}
  {#if found?.commit && found.rev === rev.trim()}
    <span class="commit truncate" title="{found.commit.oid} {found.commit.summary}">
      <span class="mono">{shortOid(found.commit.oid)}</span>
      <span class="truncate">{found.commit.summary}</span>
    </span>
  {/if}
{/snippet}

<Dialog title="Start Bisect" {onclose} onconfirm={submit} width="min(520px, 92vw)">
  <div class="form">
    <p class="lead">
      Git checks out a commit halfway between a bad and a good one. Test it and mark it good or
      bad; the search then halves again until the first bad commit is found.
    </p>

    <label class="field">
      <span class="caption">Bad commit — has the problem</span>
      <input bind:this={field} type="text" bind:value={bad} placeholder="HEAD" spellcheck="false" />
      {@render named(badCommit, bad)}
    </label>

    <label class="field">
      <span class="caption">Good commit — does not have it yet</span>
      <input type="text" bind:value={good} placeholder="A tag, a branch or an id" spellcheck="false" />
      {@render named(goodCommit, good)}
      {#if good.trim() === ""}
        <span class="hint">Leave it empty to mark a good commit later, in the graph.</span>
      {/if}
    </label>

    {#if problem}<span class="problem" role="alert">{problem}</span>{/if}
  </div>

  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null || busy} onclick={submit}>
      {busy ? "Starting…" : "Start Bisect"}
    </button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .lead {
    margin: 0;
    line-height: 1.5;
    color: var(--text-secondary);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .caption,
  .hint {
    color: var(--text-secondary);
  }

  .commit {
    display: flex;
    gap: var(--sp-3);
    min-width: 0;
  }

  .problem {
    color: var(--status-delete);
  }
</style>
