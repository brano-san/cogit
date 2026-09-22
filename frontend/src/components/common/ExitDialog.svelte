<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { untrack } from "svelte";

  /** SmartGit's Exit question. With work still queued it is also a warning, and then it
      shows whatever "Don't show again" says (requirement 1.3). */
  interface Props {
    /** One line per unfinished operation; empty when the queue is idle. */
    blockers: readonly string[];
    /** The checkbox as the setting has it, so the dialog and Settings agree. */
    dontShow: boolean;
    onexit: (dontShowAgain: boolean) => void;
    oncancel: () => void;
  }

  let { blockers, dontShow, onexit, oncancel }: Props = $props();

  // Seeded once: the dialog is created for one question and the box is then the user's.
  let checked = $state(untrack(() => dontShow));
</script>

<Dialog title="Exit" onclose={oncancel} onconfirm={() => onexit(checked)} width="min(460px, 92vw)">
  <div class="exit">
    <span class="icon" aria-hidden="true">?</span>
    <div>
      <p class="question">Do you want to exit Cogit now?</p>
      <p class="muted">By closing the last window you will exit Cogit.</p>

      {#if blockers.length > 0}
        <div class="busy" role="alert">
          <p>
            {blockers.length === 1
              ? "1 operation has not finished:"
              : `${blockers.length} operations have not finished:`}
          </p>
          <ul>
            {#each blockers as line (line)}<li>{line}</li>{/each}
          </ul>
          <p>Exiting stops them. A push or a rebase cut off half way can leave work behind.</p>
        </div>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    <label class="dont-show">
      <input type="checkbox" bind:checked />
      <span>Don't show again</span>
    </label>
    <span class="grow"></span>
    <button type="button" class="btn" onclick={oncancel}>Cancel</button>
    <button type="button" class="btn primary" onclick={() => onexit(checked)}>Exit Now</button>
  {/snippet}
</Dialog>

<style>
  .exit {
    display: flex;
    gap: var(--sp-5);
  }

  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 32px;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--status-ref);
    color: var(--text-on-accent);
    font-size: 18px;
    font-weight: 700;
  }

  .question {
    margin: 0 0 var(--sp-2);
    font-weight: 600;
  }

  .muted {
    margin: 0;
    color: var(--text-secondary);
  }

  .busy {
    margin-top: var(--sp-5);
    padding: var(--sp-3) var(--sp-4);
    border-left: 3px solid var(--status-modify);
    background: var(--surface-raised);
    border-radius: var(--r-sm);
  }

  .busy p {
    margin: 0;
  }

  .busy ul {
    margin: var(--sp-2) 0;
    padding-left: var(--sp-6);
  }

  .dont-show {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
