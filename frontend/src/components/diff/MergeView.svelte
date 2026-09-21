<script lang="ts">
  import VirtualList from "$components/common/VirtualList.svelte";
  import {
    autoResolvedCount,
    chooseAll,
    conflictRows,
    mergeRows,
    mergedText,
    nextConflict,
    syntacticCount,
    unresolvedCount,
    type Choice,
    type Choices,
  } from "$lib/merge-view";
  import type { Region } from "$lib/ipc";

  interface Props {
    path: string;
    regions: readonly Region[];
    onsave: (text: string) => void;
    oncancel: () => void;
    /** Absent in the window of its own, where there is nowhere to pop out to. */
    onpopout?: () => void;
  }

  let { path, regions, onsave, oncancel, onpopout }: Props = $props();

  let choices = $state.raw<Choices>({});
  let edited = $state<string | null>(null);
  let at = $state<number | null>(null);

  const rows = $derived(mergeRows(regions, choices));
  const conflicts = $derived(conflictRows(rows));
  const left = $derived(unresolvedCount(regions, choices));
  const auto = $derived(autoResolvedCount(regions));
  const parsed = $derived(syntacticCount(regions));
  const text = $derived(edited ?? mergedText(regions, choices));
  const current = $derived(at === null ? 0 : conflicts.indexOf(at) + 1);

  function pick(region: number, side: Choice) {
    choices = { ...choices, [region]: side };
  }

  function step(direction: 1 | -1) {
    at = nextConflict(conflicts, at, direction);
  }

  function save() {
    onsave(text);
  }

  function onkeydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.key !== "s") return;
    event.preventDefault();
    if (left === 0) save();
  }
</script>

<svelte:window {onkeydown} />

<div class="merge">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    <span class="count" class:clean={left === 0}>
      {left === 0 ? "all resolved" : `${left} unresolved`}
    </span>
    {#if auto > 0}
      <span class="auto" title="Taken without asking; look before you commit">
        {auto} resolved automatically{parsed > 0 ? `, ${parsed} by the parser` : ""} — worth a look
      </span>
    {/if}
    <span class="grow"></span>
    <span class="nav">
      <button type="button" onclick={() => step(-1)} disabled={conflicts.length === 0}>↑</button>
      <span class="tabular">{current}/{conflicts.length}</span>
      <button type="button" onclick={() => step(1)} disabled={conflicts.length === 0}>↓</button>
    </span>
    <button type="button" onclick={() => (choices = chooseAll(regions, "ours"))}>
      Take all ours
    </button>
    <button type="button" onclick={() => (choices = chooseAll(regions, "theirs"))}>
      Take all theirs
    </button>
    {#if onpopout}
      <button type="button" onclick={onpopout} title="Open in a window of its own">⧉</button>
    {/if}
    <button type="button" onclick={oncancel}>Cancel</button>
    <button type="button" class="primary" disabled={left > 0} onclick={save} title="Ctrl+S">
      Save resolution
    </button>
  </div>

  <div class="heads">
    <span>Theirs</span>
    <span>Base</span>
    <span>Ours</span>
    <span>Result</span>
  </div>

  {#if edited === null}
    <VirtualList items={rows} reveal={at} label="Merge">
      {#snippet row(entry, index)}
        <div
          class="line"
          class:conflict={entry.conflict}
          class:auto={entry.origin !== null && entry.origin !== "unchanged"}
          class:here={at !== null && entry.region === rows[at]?.region}
          style:top="{index * 22}px"
        >
          <span class="cell mono truncate">{entry.theirs ?? ""}</span>
          <span class="cell mono truncate">{entry.base ?? ""}</span>
          <span class="cell mono truncate">{entry.ours ?? ""}</span>
          <span class="cell mono truncate result">
            {entry.result ?? ""}
            {#if entry.conflict && entry.result === null && conflicts.includes(index)}
              <span class="take">
                <button type="button" onclick={() => pick(entry.region, "theirs")}>theirs</button>
                <button type="button" onclick={() => pick(entry.region, "ours")}>ours</button>
                <button type="button" onclick={() => pick(entry.region, "both")}>both</button>
              </span>
            {/if}
          </span>
        </div>
      {/snippet}
    </VirtualList>
  {:else}
    <textarea bind:value={edited} spellcheck="false" aria-label="Resolved content"></textarea>
  {/if}

  <div class="foot">
    {#if edited === null}
      <button type="button" onclick={() => (edited = mergedText(regions, choices))}>
        Edit result by hand
      </button>
    {:else}
      <button type="button" onclick={() => (edited = null)}>Back to the panels</button>
      <button type="button" class="primary" onclick={save}>Save resolution</button>
    {/if}
  </div>
</div>

<style>
  .merge {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar,
  .foot {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-5);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .foot {
    border-bottom: 0;
    border-top: 1px solid var(--divider);
  }

  .path {
    min-width: 0;
  }

  .grow {
    flex: 1 1 auto;
  }

  .count {
    color: var(--status-delete);
  }

  .count.clean {
    color: var(--status-add);
  }

  .auto {
    color: var(--status-modify);
    font-size: 11px;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: var(--sp-2, 3px);
    color: var(--text-secondary);
  }

  button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button.primary {
    background: var(--status-ref);
    color: var(--c-bg-window);
    border-color: var(--status-ref);
  }

  button:disabled {
    opacity: 0.5;
  }

  .heads {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    flex: 0 0 auto;
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .line {
    position: absolute;
    left: 0;
    right: 0;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--sp-3);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-code);
    line-height: 22px;
  }

  .line.conflict {
    background: var(--c-modified-bg);
  }

  .line.auto {
    background: var(--c-stash-bg);
  }

  .line.here {
    box-shadow: inset 0 0 0 1px var(--status-ref);
  }

  .cell {
    min-width: 0;
  }

  .result {
    display: flex;
    align-items: center;
    gap: var(--sp-2, 3px);
  }

  .take button {
    height: 16px;
    padding: 0 var(--sp-2, 3px);
    font-size: 10px;
  }

  textarea {
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: var(--sp-3) var(--sp-5);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 0;
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    resize: none;
  }
</style>
