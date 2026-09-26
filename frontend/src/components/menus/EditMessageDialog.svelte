<script lang="ts">
  import { shortOid } from "$lib/format";
  import Dialog from "$components/common/Dialog.svelte";

  interface Props {
    oid: string;
    message: string;
    onsave: (message: string) => void;
    onclose: () => void;
  }

  let { oid, message, onsave, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let text = $state(message);
  let field: HTMLTextAreaElement | undefined = $state();

  const problem = $derived(
    text.trim() === "" ? "Enter a message." : text.trim() === message.trim() ? "Nothing changed yet." : null,
  );

  function submit() {
    if (problem === null) onsave(text.replace(/\s+$/, ""));
  }

  $effect(() => {
    field?.focus();
  });
</script>

<Dialog title="Edit Message of {shortOid(oid)}" {onclose} dirty={text.trim() !== message.trim()} width="min(600px, 92vw)">
  <textarea bind:this={field} bind:value={text} rows="10" aria-label="Commit message"></textarea>

  {#snippet footer()}
    {#if problem && text.trim() !== message.trim()}<span class="problem">{problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>Save</button>
  {/snippet}
</Dialog>

<style>
  textarea {
    width: 100%;
    min-height: 180px;
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
    resize: vertical;
  }

  .problem {
    color: var(--status-delete);
    font-size: var(--fs-dense);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
