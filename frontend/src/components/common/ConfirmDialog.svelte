<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";

  interface Props {
    title: string;
    message: string;
    confirm: string;
    warning?: boolean;
    onanswer: (yes: boolean) => void;
  }

  let { title, message, confirm, warning = false, onanswer }: Props = $props();
</script>

<Dialog {title} onclose={() => onanswer(false)} onconfirm={() => onanswer(true)} width="min(460px, 90vw)">
  <p class="message">{message}</p>

  {#snippet footer()}
    <!-- A destructive question starts on Cancel: Enter must not be the way work is lost. -->
    <button class="btn" type="button" data-autofocus={warning || undefined} onclick={() => onanswer(false)}>
      Cancel
    </button>
    <button
      type="button"
      class="btn"
      class:primary={!warning}
      class:warning
      data-autofocus={!warning || undefined}
      onclick={() => onanswer(true)}
    >
      {confirm}
    </button>
  {/snippet}
</Dialog>

<style>
  .message {
    margin: 0;
    font-size: var(--fs-dense);
    line-height: 1.5;
    white-space: pre-line;
    overflow-wrap: anywhere;
  }
</style>
