<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";

  interface Props {
    title: string;
    message: string;
    confirm: string;
    warning?: boolean;
    option?: string;
    onanswer: (yes: boolean, checked: boolean) => void;
  }

  let { title, message, confirm, warning = false, option, onanswer }: Props = $props();
  let checked = $state(false);
</script>

<Dialog
  {title}
  onclose={() => onanswer(false, false)}
  onconfirm={() => onanswer(true, checked)}
  width="min(460px, 90vw)"
>
  <p class="message">{message}</p>
  {#if option}
    <label class="option"><input type="checkbox" bind:checked /> {option}</label>
  {/if}

  {#snippet footer()}
    <!-- A destructive question starts on Cancel: Enter must not be the way work is lost. -->
    <button class="btn" type="button" data-autofocus={warning || undefined} onclick={() => onanswer(false, false)}>
      Cancel
    </button>
    <button
      type="button"
      class="btn"
      class:primary={!warning}
      class:warning
      data-autofocus={!warning || undefined}
      onclick={() => onanswer(true, checked)}
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
  .option {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 10px;
    font-size: var(--fs-dense);
  }
</style>
