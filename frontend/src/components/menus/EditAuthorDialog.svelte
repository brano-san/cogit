<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { authorProblem } from "$lib/rewrite-plans";

  interface Props {
    oid: string;
    name: string;
    email: string;
    onsave: (name: string, email: string) => void;
    onclose: () => void;
  }

  let { oid, name: initialName, email: initialEmail, onsave, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(initialName);
  // svelte-ignore state_referenced_locally
  let email = $state(initialEmail);
  let field: HTMLInputElement | undefined = $state();

  const problem = $derived(
    authorProblem(name, email) ??
      (name.trim() === initialName && email.trim() === initialEmail ? "Nothing changed yet." : null),
  );

  function submit() {
    if (problem === null) onsave(name.trim(), email.trim());
  }

  $effect(() => {
    field?.focus();
    field?.select();
  });
</script>

<Dialog title="Edit Author of {oid.slice(0, 7)}" {onclose} onconfirm={submit} width="min(480px, 92vw)">
  <div class="form">
    <label class="field">
      <span class="caption">Name</span>
      <input bind:this={field} type="text" bind:value={name} />
    </label>
    <label class="field">
      <span class="caption">Email</span>
      <input type="text" bind:value={email} />
    </label>
    <p class="hint">The committer becomes you; the author date stays as it was.</p>
  </div>

  {#snippet footer()}
    {#if problem && problem !== "Nothing changed yet."}<span class="problem">{problem}</span>{/if}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>Save</button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .caption,
  .hint {
    color: var(--text-secondary);
  }

  .hint {
    margin: 0;
  }

  .problem {
    color: var(--status-delete);
    font-size: var(--fs-dense);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
