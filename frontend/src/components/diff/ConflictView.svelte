<script lang="ts">
  import type { ConflictSide } from "$lib/ipc";

  interface Props {
    path: string;
    base: string | null;
    ours: string | null;
    theirs: string | null;
    /** Binary or not UTF-8: the text above is a rendering, and writing it back corrupts. */
    binary?: boolean;
    onresolve: (side: ConflictSide) => void;
    onresolveText: (text: string) => void;
  }

  let { path, base, ours, theirs, binary = false, onresolve, onresolveText }: Props = $props();

  let editing = $state(false);
  let draft = $state("");

  const sides: { id: ConflictSide; label: string; text: string | null }[] = $derived([
    { id: "base", label: "Base", text: base },
    { id: "ours", label: "Ours", text: ours },
    { id: "theirs", label: "Theirs", text: theirs },
  ]);

  function startEditing(from: string | null) {
    draft = from ?? "";
    editing = true;
  }

  function save() {
    onresolveText(draft);
    editing = false;
  }
</script>

<div class="conflict">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    <span class="warn">conflicted</span>
    <span class="grow"></span>
    {#if editing}
      <button type="button" onclick={() => (editing = false)}>Cancel</button>
      <button type="button" onclick={save}>Save resolution</button>
    {:else}
      <button
        type="button"
        disabled={binary}
        title={binary ? "Binary or not UTF-8: take one side whole" : undefined}
        onclick={() => startEditing(ours)}>Edit by hand</button
      >
      <button type="button" disabled={ours === null} onclick={() => onresolve("ours")}>
        Take ours
      </button>
      <button type="button" disabled={theirs === null} onclick={() => onresolve("theirs")}>
        Take theirs
      </button>
    {/if}
  </div>

  {#if editing}
    <textarea bind:value={draft} spellcheck="false" aria-label="Resolved content"></textarea>
  {:else}
    <div class="columns">
      {#each sides as side (side.id)}
        <div class="column">
          <div class="head">{side.label}</div>
          {#if side.text === null}
            <p class="message">Absent on this side.</p>
          {:else if binary}
            <p class="message">Binary or not UTF-8 — take one side whole.</p>
          {:else}
            <pre class="body mono">{side.text}</pre>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .conflict {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .path {
    flex: 0 1 auto;
    min-width: 0;
  }

  .warn {
    color: var(--status-delete);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .grow {
    flex: 1 1 auto;
  }

  .bar button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .bar button:disabled {
    opacity: 0.45;
  }

  .bar button:not(:disabled):hover {
    border-color: var(--status-ref);
  }

  .columns {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .column {
    display: flex;
    flex-direction: column;
    flex: 1 1 33%;
    min-width: 0;
    border-right: 1px solid var(--divider);
  }

  .column:last-child {
    border-right: 0;
  }

  .head {
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .body {
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: var(--sp-4);
    overflow: auto;
    font-size: var(--fs-code);
    white-space: pre;
    user-select: text;
  }

  textarea {
    flex: 1 1 auto;
    min-height: 0;
    margin: var(--sp-4);
    padding: var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    resize: none;
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
