<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import type { ResolveChoice, ResolveClose } from "$lib/solver-flow";

  /** Closing the solver on a file that is still conflicted: stage it, leave it conflicted,
      or stay; unsaved edits are part of the same question (resolveClose). */
  interface Props {
    path: string;
    question: ResolveClose;
    onanswer: (choice: ResolveChoice) => void;
  }

  let { path, question, onanswer }: Props = $props();
</script>

<Dialog
  title="Close Conflicted File"
  onclose={() => onanswer("cancel")}
  onconfirm={() => onanswer(question.primary)}
  width="min(520px, 90vw)"
>
  <p class="message">{path} is still marked as conflicted. Mark it resolved and stage the Result, or leave it conflicted?</p>
  {#if question.warning}<p class="message warning" role="alert">{question.warning}</p>{/if}

  {#snippet footer()}
    {#each question.choices as { choice, label } (choice)}
      <button
        class="btn"
        class:primary={choice === question.primary}
        class:warning={choice === "discard"}
        type="button"
        data-autofocus={choice === question.primary ? true : undefined}
        onclick={() => onanswer(choice)}>{label}</button
      >
    {/each}
  {/snippet}
</Dialog>

<style>
  .message {
    margin: 0;
    font-size: var(--fs-dense);
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .message.warning {
    margin-top: var(--sp-3);
    color: var(--badge-warning-fg);
  }
</style>
