<script lang="ts">
  import { shortOid } from "$lib/format";

  /** What changed in a submodule is which commit the parent records — not any file. A
      submodule nobody checked out has nothing else to show, and saying so is the answer,
      not an error (doc/12-risks.md, R-139). */
  interface Props {
    path: string;
    /** `null` on the side that removed the submodule. */
    recorded: string | null;
    previous: string | null;
    checkedOut: boolean;
    /** `git submodule update --init` needs the gitlink in the index. */
    inIndex: boolean;
    /** Absent in a window of its own: initialising is a write, and writes queue in the
        main window. */
    oninit?: () => void;
  }

  let { path, recorded, previous, checkedOut, inIndex, oninit }: Props = $props();
</script>

<div class="submodule">
  {#if recorded === null}
    <p class="what"><span class="mono">{path}</span> was a submodule; this removes it.</p>
  {:else}
    <p class="what"><span class="mono">{path}</span> is a submodule.</p>
  {/if}

  <dl>
    {#if previous}
      <dt>Was</dt>
      <dd class="mono">{shortOid(previous)}</dd>
    {/if}
    {#if recorded !== null}
      <dt>Now</dt>
      <dd class="mono">{shortOid(recorded)}</dd>
    {/if}
  </dl>

  {#if recorded === null}
    <!-- Nothing to initialise or open: the parent no longer records it. -->
  {:else if checkedOut}
    <p class="note">Open it from the Repositories panel to see what those commits are.</p>
  {:else if inIndex}
    <p class="note">
      It has never been checked out, so its commits are not here to show.
    </p>
    {#if oninit}
      <button type="button" class="btn" onclick={oninit}>Initialise this submodule</button>
    {:else}
      <p class="note">Initialise it from the Diff panel of the main window.</p>
    {/if}
  {:else}
    <p class="note">
      It is not checked out, and the index no longer records it, so there is nothing to
      initialise.
    </p>
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
