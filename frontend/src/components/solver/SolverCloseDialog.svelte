<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import type { CloseChoice } from "$lib/solver-flow";

  interface Props {
    path: string;
    onanswer: (choice: CloseChoice) => void;
  }

  let { path, onanswer }: Props = $props();
</script>

<Dialog title="Unsaved Resolution" onclose={() => onanswer("cancel")} onconfirm={() => onanswer("save")} width="min(460px, 90vw)">
  <p class="message">The edits to {path} are not saved. Save them and stage the file, or throw them away?</p>

  {#snippet footer()}
    <button class="btn" type="button" onclick={() => onanswer("cancel")}>Cancel</button>
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
