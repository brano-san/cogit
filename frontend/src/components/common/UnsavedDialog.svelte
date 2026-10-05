<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import type { CloseChoice } from "$lib/solver-flow";

  /** Edits not on disk and something about to drop them: Save, Discard, or stay (Cancel).
      `cancellable` off when the editor is going whatever the answer (another repository). */
  interface Props {
    title: string;
    path: string;
    cancellable?: boolean;
    onanswer: (choice: CloseChoice) => void;
  }

  let { title, path, cancellable = true, onanswer }: Props = $props();
</script>

<Dialog
  {title}
  onclose={() => onanswer(cancellable ? "cancel" : "discard")}
  onconfirm={() => onanswer("save")}
  width="min(460px, 90vw)"
>
  <p class="message">The edits to {path} are not saved. Save them, or throw them away?</p>

  {#snippet footer()}
    {#if cancellable}<button class="btn" type="button" onclick={() => onanswer("cancel")}>Cancel</button>{/if}
    <button class="btn warning" type="button" onclick={() => onanswer("discard")}>Discard</button>
    <button class="btn primary" type="button" data-autofocus onclick={() => onanswer("save")}>Save</button>
  {/snippet}
</Dialog>

<style>
  .message {
    margin: 0;
    font-size: var(--fs-dense);
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
</style>
