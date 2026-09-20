<script lang="ts">
  import { accelerator, conflicts, effective, prettyKeys, type Keymap } from "$lib/keymap";
  import type { KeyBinding } from "$lib/ipc";

  interface Props {
    bindings: readonly KeyBinding[];
    overrides: Keymap;
    onchange: (next: Keymap) => void;
  }

  let { bindings, overrides, onchange }: Props = $props();

  let capturing = $state<string | null>(null);
  let filter = $state("");

  const onMac = typeof navigator !== "undefined" && navigator.platform.startsWith("Mac");
  const keys = $derived(effective(bindings, overrides));
  const clashes = $derived(conflicts(bindings, overrides));
  const shown = $derived(
    bindings.filter(
      (binding) =>
        filter.trim() === "" ||
        `${binding.section} ${binding.label}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );

  function capture(event: KeyboardEvent, id: string) {
    event.preventDefault();
    if (event.key === "Escape") {
      capturing = null;
      return;
    }
    const chosen = accelerator(event);
    if (chosen === null) return;
    capturing = null;
    onchange({ ...overrides, [id]: chosen });
  }

  function clear(id: string) {
    onchange({ ...overrides, [id]: "" });
  }

  function restore(id: string) {
    const next = { ...overrides };
    delete next[id];
    onchange(next);
  }
</script>

<div class="keymap">
  <input
    type="search"
    bind:value={filter}
    placeholder="Filter commands"
    aria-label="Filter commands"
  />

  <div class="rows">
    {#each shown as binding (binding.id)}
      {@const clash = clashes[binding.id]}
      <div class="row" class:clash={clash !== undefined}>
        <span class="section">{binding.section}</span>
        <span class="label truncate">{binding.label}</span>
        <button
          type="button"
          class="keys"
          class:capturing={capturing === binding.id}
          onclick={() => (capturing = capturing === binding.id ? null : binding.id)}
          onkeydown={(event) => capturing === binding.id && capture(event, binding.id)}
          title={clash ? `Also bound to ${clash.join(", ")}` : "Click, then press the keys"}
        >
          {capturing === binding.id ? "Press keys…" : prettyKeys(keys[binding.id] ?? "", onMac)}
        </button>
        <button type="button" class="act" title="Remove the key" onclick={() => clear(binding.id)}
          >✕</button
        >
        <button
          type="button"
          class="act"
          title="Back to the shipped key"
          disabled={overrides[binding.id] === undefined}
          onclick={() => restore(binding.id)}>↺</button
        >
      </div>
    {/each}
  </div>
</div>

<style>
  .keymap {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-height: 0;
  }

  .rows {
    display: flex;
    flex-direction: column;
    max-height: 46vh;
    overflow: auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-2);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.clash .keys {
    border-color: var(--status-delete);
    color: var(--status-delete);
  }

  .section {
    flex: 0 0 86px;
    color: var(--text-secondary);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .label {
    flex: 1 1 auto;
    min-width: 0;
  }

  .keys {
    flex: 0 0 160px;
    height: 20px;
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    font-family: var(--font-mono);
    cursor: default;
  }

  .keys.capturing {
    border-color: var(--status-ref);
    color: var(--status-ref);
  }

  .act {
    flex: 0 0 auto;
    width: 20px;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: default;
  }

  .act:disabled {
    opacity: 0.3;
  }

  .act:hover:not(:disabled) {
    color: var(--status-ref);
  }
</style>
