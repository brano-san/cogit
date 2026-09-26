<script lang="ts">
  import { ON_MAC, claimable, conflicts, effective, prettyKeys, recordKeys, type Keymap } from "$lib/keymap";
  import { captureKeys, type KeyBinding } from "$lib/ipc";

  interface Props {
    bindings: readonly KeyBinding[];
    overrides: Keymap;
    onchange: (next: Keymap) => void;
  }

  let { bindings, overrides, onchange }: Props = $props();

  let capturing = $state<string | null>(null);
  /** Why the last key pressed while capturing was not taken. */
  let refusal = $state<string | null>(null);
  let filter = $state("");

  // The menu takes a key it has before the page sees it, and runs its command instead.
  $effect(() => {
    if (capturing === null) return;
    void captureKeys(true);
    return () => void captureKeys(false);
  });

  const onMac = ON_MAC;
  const keys = $derived(effective(bindings, overrides));
  const clashes = $derived(conflicts(bindings, overrides));
  const shown = $derived(
    bindings.filter(
      (binding) =>
        filter.trim() === "" ||
        `${binding.section} ${binding.label}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );

  function startOrStop(id: string) {
    capturing = capturing === id ? null : id;
    refusal = null;
  }

  function capture(event: KeyboardEvent, id: string) {
    // Tab leaves the button, as it does everywhere else.
    if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      capturing = null;
      return;
    }
    event.preventDefault();
    if (event.key === "Escape") {
      capturing = null;
      return;
    }
    const recorded = recordKeys(event, onMac);
    if (recorded === null) return;
    if ("refused" in recorded) {
      refusal = recorded.refused;
      return;
    }
    capturing = null;
    refusal = null;
    onchange({ ...overrides, [id]: recorded.keys });
  }

  /** A key recorded before the editor checked, which the window never runs. */
  function dead(keys: string): boolean {
    return keys !== "" && !claimable(keys);
  }

  function title(id: string): string {
    const clash = clashes[id];
    if (clash) return `Also bound to ${clash.join(", ")}`;
    if (dead(keys[id] ?? "")) return "Cogit cannot take this key; press another";
    return "Click, then press the keys";
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
      <div class="row" class:clash={clashes[binding.id] !== undefined || dead(keys[binding.id] ?? "")}>
        <span class="section">{binding.section}</span>
        <span class="label truncate">{binding.label}</span>
        <button
          type="button"
          class="keys"
          class:capturing={capturing === binding.id}
          onclick={() => startOrStop(binding.id)}
          onkeydown={(event) => capturing === binding.id && capture(event, binding.id)}
          onblur={() => capturing === binding.id && (capturing = null)}
          title={title(binding.id)}
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

  <p class="hint" role="status">{capturing !== null && refusal ? refusal : ""}</p>
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

  /* Holds its line even when empty, so the list does not jump when a key is refused. */
  .hint {
    min-height: 1.4em;
    margin: 0;
    color: var(--status-modify);
    font-size: var(--fs-header);
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
    height: var(--h-button-sm);
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
