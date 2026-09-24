<script lang="ts">
  import { shortOid } from "$lib/format";
  import Dialog from "$components/common/Dialog.svelte";
  import { RESET_CHOICES } from "$lib/reset-modes";
  import type { ResetMode } from "$lib/ipc/ref-ops";

  /** Reset Advanced…: the five modes, each with what it does to the index and the tree. */
  interface Props {
    /** What moves: the checked-out branch, or HEAD when it is detached. */
    moving: string;
    oid: string;
    subject: string;
    onreset: (mode: ResetMode) => void;
    onclose: () => void;
  }

  let { moving, oid, subject, onreset, onclose }: Props = $props();

  let mode = $state<ResetMode>("mixed");
  const chosen = $derived(RESET_CHOICES.find((choice) => choice.mode === mode));
</script>

<Dialog title="Reset" {onclose} onconfirm={() => onreset(mode)} width="min(560px, 92vw)">
  <div class="form">
    <p class="what">
      Reset <span class="mono">{moving}</span> to
      <span class="mono">{shortOid(oid)}</span>
      <span class="subject">{subject}</span>
    </p>

    <fieldset>
      <legend class="caption">Mode</legend>
      {#each RESET_CHOICES as choice (choice.mode)}
        <label class="choice">
          <input
            type="radio"
            name="reset-mode"
            checked={mode === choice.mode}
            onchange={() => (mode = choice.mode)}
          />
          <span class="text">
            <span class="label">{choice.label}</span>
            <span class="explanation">{choice.explanation}</span>
          </span>
        </label>
      {/each}
    </fieldset>
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button
      type="button"
      class="btn"
      class:primary={!chosen?.destructive}
      class:warning={chosen?.destructive}
      onclick={() => onreset(mode)}
    >
      {chosen?.destructive ? "Reset Hard…" : "Reset"}
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

  .what {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .subject {
    color: var(--text-secondary);
  }

  .caption {
    color: var(--text-secondary);
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    padding: 0;
    margin-bottom: var(--sp-2);
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }

  .choice input {
    margin-top: 2px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }

  .label {
    font-weight: 600;
  }

  .explanation {
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
