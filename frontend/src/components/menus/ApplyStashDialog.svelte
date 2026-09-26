<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";

  /** Apply Stash (item 40): a double click on a stash and the menu's Apply Stash. */
  interface Props {
    index: number;
    message: string;
    onapply: (drop: boolean, restoreIndex: boolean) => void;
    onclose: () => void;
  }

  let { index, message, onapply, onclose }: Props = $props();

  let restoreIndex = $state(false);
</script>

<Dialog title="Apply Stash" {onclose} onconfirm={() => onapply(true, restoreIndex)} width="min(500px, 92vw)">
  <div class="form">
    <p class="what">
      Apply <span class="mono">{`stash@{${index}}`}</span>
      {#if message}<span class="subject">{message}</span>{/if}
      to the working tree.
    </p>
    <Checkbox
      bind:checked={restoreIndex}
      label="Restore Index"
      title="Staged changes come back staged; git refuses when the index cannot be restored"
    />
    <p class="explanation">
      Apply & Drop removes the stash only once it applied without conflicts; with conflicts it
      stays in the list.
    </p>
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn" onclick={() => onapply(false, restoreIndex)}>Apply</button>
    <button type="button" class="btn primary" data-autofocus onclick={() => onapply(true, restoreIndex)}
      >Apply & Drop</button
    >
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    min-width: 0;
    font-size: var(--fs-dense);
  }

  .what {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .subject,
  .explanation {
    color: var(--text-secondary);
  }

  .explanation {
    margin: 0;
    line-height: 1.4;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
