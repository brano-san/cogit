<script lang="ts">
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
  let field: HTMLInputElement | HTMLSelectElement | undefined = $state();

  const problem = $derived(validate?.(text) ?? (text.trim() === "" ? "Enter a value." : null));

  function submit() {
    if (problem === null) onaccept(text.trim());
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "Enter") {
      event.preventDefault();
      submit();
    }
  }

  $effect(() => {
    field?.focus();
    if (field instanceof HTMLInputElement) field.select();
  });
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="dialog" role="dialog" aria-label={title}>
  <header><h2>{title}</h2></header>

  <label class="field">
    <span>{label}</span>
    {#if choices}
      <select bind:this={field} bind:value={text}>
        {#each choices as choice (choice)}<option value={choice}>{choice}</option>{/each}
      </select>
    {:else}
      <input bind:this={field} type="text" bind:value={text} />
    {/if}
  </label>

  <footer>
    {#if problem}<span class="problem">{problem}</span>{/if}
    <button type="button" onclick={onclose}>Cancel</button>
    <button type="button" class="primary" disabled={problem !== null} onclick={submit}>
      {confirm}
    </button>
  </footer>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--scrim);
  }

  .dialog {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 21;
    width: min(440px, 90vw);
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
  }

  header {
    height: var(--h-toolbar);
    display: flex;
    align-items: center;
    padding: 0 var(--sp-5);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .field input,
  .field select {
    height: var(--h-input);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
  }

  .problem {
    flex: 1 1 auto;
    color: var(--status-delete);
    font-size: 11px;
  }

  footer button:first-of-type {
    margin-left: auto;
  }

  .problem ~ button:first-of-type {
    margin-left: 0;
  }

  .primary {
    border-color: var(--status-ref);
  }
</style>
