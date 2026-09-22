<script lang="ts">
  import { shortOid } from "$lib/format";

  /** What changed in a submodule is which commit the parent records — not any file. A
      submodule nobody checked out has nothing else to show, and saying so is the answer,
      not an error (doc/12-risks.md, R-139). */
  interface Props {
    path: string;
    recorded: string;
    previous: string | null;
    checkedOut: boolean;
    oninit: () => void;
  }

  let { path, recorded, previous, checkedOut, oninit }: Props = $props();
</script>

<div class="submodule">
  <p class="what"><span class="mono">{path}</span> is a submodule.</p>

  <dl>
    {#if previous}
      <dt>Was</dt>
      <dd class="mono">{shortOid(previous)}</dd>
    {/if}
    <dt>Now</dt>
    <dd class="mono">{shortOid(recorded)}</dd>
  </dl>

  {#if checkedOut}
    <p class="note">Open it from the Repositories panel to see what those commits are.</p>
  {:else}
    <p class="note">
      It has never been checked out, so its commits are not here to show.
    </p>
    <button type="button" class="btn" onclick={oninit}>Initialise this submodule</button>
  {/if}
</div>

<style>
  .submodule {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-4);
    padding: var(--sp-6) var(--sp-5);
    font-size: var(--fs-dense);
  }

  .what {
    margin: 0;
    color: var(--text-primary);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--sp-2) var(--sp-4);
    margin: 0;
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    color: var(--text-primary);
    user-select: text;
  }

  .note {
    margin: 0;
    color: var(--text-secondary);
  }

  .btn {
    height: var(--h-button);
    padding: 0 var(--sp-5);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  .btn:hover {
    border-color: var(--state-focus-ring);
  }
</style>
