<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import type { AutostashAnswer } from "$stores/autostash-dialog.svelte";

  /** Git refused a checkout for local changes: carry them over in a stash (item 46). */
  interface Props {
    question: string;
    onanswer: (answer: AutostashAnswer | null) => void;
  }

  let { question, onanswer }: Props = $props();

  let drop = $state(true);
</script>

<Dialog title="Check Out" onclose={() => onanswer(null)} onconfirm={() => onanswer({ drop })} width="min(480px, 90vw)">
  <div class="form">
    <p class="message">{question}</p>
    <Checkbox bind:checked={drop} label="Drop the stash once it applies cleanly" />
    <p class="explanation">
      The stash is out of the list while the checkout runs. If the changes conflict, it stays
      there as stash@{"{0}"}.
    </p>
  </div>

  {#snippet footer()}
    <button class="btn" type="button" onclick={() => onanswer(null)}>Cancel</button>
    <button type="button" class="btn primary" data-autofocus onclick={() => onanswer({ drop })}
      >Stash and Check Out</button
    >
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .message,
  .explanation {
    margin: 0;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .explanation {
    color: var(--text-secondary);
  }
</style>
