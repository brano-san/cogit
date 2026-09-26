<script lang="ts">
  import { modals } from "$lib/modal-stack";
  import VirtualList from "$components/common/VirtualList.svelte";
  import SidewaysScrollbar from "$components/common/SidewaysScrollbar.svelte";
  import { TRAILING_COLUMNS, clampOffset, maxOffset, textColumns, wheelSideways } from "$lib/code-scroll";
  import {
    CHOICES,
    autoResolvedCount,
    canSave,
    chooseAll,
    clearChoice,
    conflictRows,
    editableText,
    mergeKey,
    mergeRows,
    mergedText,
    nextConflict,
    panelConflictStep,
    syntacticCount,
    unresolvedCount,
    unsavedResolution,
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
    /** Only the separate window may take its keys (Ctrl+S, F6, Ctrl+1…3): in the main one
        the native menu owns Ctrl+S for Stash All and Ctrl+1…7 for the panels, and an
        accelerator cannot be preventDefault-ed from here. */
    saveShortcut?: boolean;
    /** Whether there are sides picked or edits not saved yet, whenever that changes. */
    onunsaved?: (unsaved: boolean) => void;
    /** In the main window: the Diff panel has the focus, so F6 steps through the conflicts. */
    active?: boolean;
  }

  let {
    path,
    regions,
    onsave,
    oncancel,
    onpopout,
    saveShortcut = false,
    onunsaved,
    active = false,
  }: Props = $props();

  let choices = $state.raw<Choices>({});
  let edited = $state<string | null>(null);
  let at = $state<number | null>(null);

  const rows = $derived(mergeRows(regions, choices));
  const conflicts = $derived(conflictRows(rows));
  const left = $derived(unresolvedCount(regions, choices));
  const auto = $derived(autoResolvedCount(regions));
  const parsed = $derived(syntacticCount(regions));
  const text = $derived(edited ?? mergedText(regions, choices));
  const saveable = $derived(canSave(regions, choices, edited));
  const current = $derived(at === null ? 0 : conflicts.indexOf(at) + 1);

  /** The four columns move sideways together, as the two halves of a diff do (R-470). */
  const PROBE = "0".repeat(100);
  let cellWidth = $state(0);
  let probeWidth = $state(0);
  let sideways = $state(0);
  const widest = $derived.by(() => {
    let most = 0;
    for (const row of rows) {
      for (const text of [row.theirs, row.base, row.ours, row.result]) {
        if (text) most = Math.max(most, textColumns(text));
      }
    }
    return most + TRAILING_COLUMNS;
  });
  const sidewaysMax = $derived(maxOffset(widest, probeWidth / PROBE.length, cellWidth));
  const shift = $derived(clampOffset(sideways, sidewaysMax));

  function onwheel(event: WheelEvent) {
    const delta = wheelSideways(event, 22);
    if (delta === 0 || sidewaysMax === 0) return;
    event.preventDefault();
    sideways = clampOffset(shift + delta, sidewaysMax);
  }

  function pick(region: number, side: Choice) {
    choices = { ...choices, [region]: side };
  }

  function step(direction: 1 | -1) {
    at = nextConflict(conflicts, at, direction);
  }

  function save() {
    if (saveable) onsave(text);
  }

  /** Read by the merge window before it closes (04 §7). */
  export function unsaved(): boolean {
    return unsavedResolution(choices, edited);
  }

  $effect(() => onunsaved?.(unsavedResolution(choices, edited)));

  /** The conflict under the cursor, or the first one when none is. */
  function take(side: Choice) {
    const row = at ?? nextConflict(conflicts, null, 1);
    const region = row === null ? undefined : rows[row]?.region;
    if (region === undefined) return;
    pick(region, side);
    at = row;
    step(1);
  }

  function onkeydown(event: KeyboardEvent) {
    if (!saveShortcut || modals.any) return;
    const action = mergeKey({
      key: event.key,
      code: event.code,
      ctrl: event.ctrlKey || event.metaKey,
      shift: event.shiftKey,
      alt: event.altKey,
    });
    if (action === null) return;
    event.preventDefault();
    if (action === "save") save();
    else if ("step" in action) step(action.step);
    // Hand-edited text is the result now; a side picked would not show in it.
    else if (edited !== null) return;
    else if (action.all) choices = chooseAll(regions, action.take);
    else take(action.take);
  }

  /** In the capture phase, ahead of the main window's panel walk, as the diff's F6 is. */
  function onpanelkey(event: KeyboardEvent) {
    if (saveShortcut || !active || modals.any) return;
    const by = panelConflictStep(
      { key: event.key, ctrl: event.ctrlKey || event.metaKey, shift: event.shiftKey, alt: event.altKey },
      conflicts,
      at,
    );
    if (by === null) return;
    event.preventDefault();
    step(by);
  }
</script>

<svelte:window {onkeydown} onkeydowncapture={onpanelkey} />

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
    <button
      type="button"
      class="primary"
      disabled={!saveable}
      onclick={save}
      title={saveShortcut ? "Ctrl+S" : "Write the resolution and stage the file"}
    >
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
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="panes" style:--shift="{shift}px" {onwheel}>
      <div class="line ruler" aria-hidden="true">
        <span class="cell mono" bind:clientWidth={cellWidth}
          ><span class="probe" bind:offsetWidth={probeWidth}>{PROBE}</span></span
        >
      </div>
      <VirtualList items={rows} reveal={at} label="Merge">
        {#snippet row(entry, index)}
          <div
            class="line"
            class:conflict={entry.conflict}
            class:auto={entry.origin !== null && entry.origin !== "unchanged"}
            class:here={at !== null && entry.region === rows[at]?.region}
            style:top="{index * 22}px"
          >
            <span class="cell mono"><span class="text">{entry.theirs ?? ""}</span></span>
            <span class="cell mono"><span class="text">{entry.base ?? ""}</span></span>
            <span class="cell mono"><span class="text">{entry.ours ?? ""}</span></span>
            <span class="cell mono result"
              ><span class="text">{entry.result ?? ""}</span
              >{#if entry.conflict && conflicts.includes(index)}
                {@const chosen = choices[entry.region]}
                <!-- On every conflict, decided or not: a side picked by mistake is changed here. -->
                <span class="take">
                  {#each CHOICES as side (side)}
                    <button
                      type="button"
                      class:active={chosen === side}
                      aria-pressed={chosen === side}
                      onclick={() => pick(entry.region, side)}>{side}</button
                    >
                  {/each}
                  {#if chosen}
                    <button
                      type="button"
                      title="Leave this conflict undecided"
                      onclick={() => (choices = clearChoice(choices, entry.region))}>✕</button
                    >
                  {/if}
                </span>
              {/if}
            </span>
          </div>
        {/snippet}
      </VirtualList>
    </div>
    <SidewaysScrollbar offset={shift} max={sidewaysMax} onscroll={(offset) => (sideways = offset)} />
  {:else}
    <textarea bind:value={edited} spellcheck="false" aria-label="Resolved content"></textarea>
  {/if}

  <div class="foot">
    {#if edited === null}
      <button type="button" onclick={() => (edited = editableText(regions, choices))}>
        Edit result by hand
      </button>
    {:else}
      <button type="button" onclick={() => (edited = null)}>Back to the panels</button>
      {#if !saveable}<span class="hint">Remove every conflict marker to save</span>{/if}
      <button type="button" class="primary" disabled={!saveable} onclick={save}>Save resolution</button>
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

  .hint {
    color: var(--text-secondary);
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
    gap: var(--sp-2);
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

  .panes {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
  }

  /* Clipped, not cut with an ellipsis: a conflict that differs at the end of a line
     scrolls into view (R-470). */
  .cell {
    min-width: 0;
    overflow: hidden;
    white-space: pre;
  }

  .text {
    display: inline-block;
    vertical-align: top;
    transform: translateX(calc(-1 * var(--shift, 0px)));
  }

  /* The layout of a row, never seen: it measures a column and one character. */
  .ruler {
    top: 0;
    visibility: hidden;
    pointer-events: none;
  }

  .probe {
    display: inline-block;
  }

  .result {
    position: relative;
  }

  /* Over the end of the result, where scrolling does not move it. */
  .take {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding-left: var(--sp-2);
    background: var(--surface-panel);
  }

  .take button {
    height: 16px;
    padding: 0 var(--sp-2);
    font-size: 10px;
  }

  /* The side this conflict takes now; the others stay, to change it. */
  .take button.active {
    color: var(--status-ref);
    border-color: var(--status-ref);
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
