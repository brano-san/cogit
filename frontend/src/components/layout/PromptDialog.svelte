<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Select from "$components/common/Select.svelte";

  /** One line of input or one choice from a list — the two things `ask` cannot do. */
  interface Props {
    title: string;
    label: string;
    value?: string;
    /** When given, the field is a list instead of free text. */
    choices?: readonly string[];
    confirm?: string;
    /** Returns why the value is unusable, or null when it will do. */
    validate?: (value: string) => string | null;
    onaccept: (value: string) => void;
    onclose: () => void;
  }

  let {
    title,
    label,
    value = "",
    choices,
    confirm = "OK",
    validate,
    onaccept,
    onclose,
  }: Props = $props();

  // svelte-ignore state_referenced_locally
  let text = $state(value || choices?.[0] || "");
  let field: HTMLInputElement | undefined = $state();

  const problem = $derived(validate?.(text) ?? (text.trim() === "" ? "Enter a value." : null));

  function submit() {
    if (problem === null) onaccept(text.trim());
  }

  $effect(() => {
    field?.focus();
    field?.select();
  });
</script>

<Dialog {title} {onclose} onconfirm={submit}>
  <label class="field">
    <span>{label}</span>
    {#if choices}
      <Select
        value={text}
        options={choices.map((choice) => [choice, choice] as const)}
        {label}
        onchange={(next) => (text = next)}
      />
    {:else}
      <input bind:this={field} type="text" bind:value={text} />
    {/if}
  </label>

  {#snippet footer()}
    {#if problem}<span class="problem">{problem}</span>{/if}
    <span class="grow"></span>
    <button class="btn" type="button" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={problem !== null} onclick={submit}>
      {confirm}
    </button>
  {/snippet}
</Dialog>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .problem {
    color: var(--status-delete);
    font-size: 11px;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
