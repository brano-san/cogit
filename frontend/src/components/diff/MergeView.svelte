<script lang="ts">
  import { modals } from "$lib/modal-stack";
  import VirtualList from "$components/common/VirtualList.svelte";
  import SidewaysScrollbar from "$components/common/SidewaysScrollbar.svelte";
  import { TRAILING_COLUMNS, clampOffset, maxOffset, textColumns, wheelSideways } from "$lib/code-scroll";
  import {
    autoResolvedCount,
    conflictCount,
    conflictRows,
    nextConflict,
    panelConflictStep,
    previewRows,
    syntacticCount,
  } from "$lib/merge-view";
  import { conflictsLeftLabel } from "$lib/solver-model";
  import type { ConflictSide, Region } from "$lib/ipc";

  /** The preview of a conflicted file in the Diff panel: Ours, the merge's own Result, and
      Theirs, none of them editable. Deciding is the Conflict Solver's job. */
  interface Props {
    path: string;
    regions: readonly Region[];
    onresolve: (side: ConflictSide) => void;
    /** Opens the Conflict Solver for this file. */
    onsolver: () => void;
    oncancel: () => void;
    /** The Diff panel has the focus, so F6 steps through the conflicts. */
    active?: boolean;
  }

  let { path, regions, onresolve, onsolver, oncancel, active = false }: Props = $props();

  let at = $state<number | null>(null);

  const rows = $derived(previewRows(regions));
  const conflicts = $derived(conflictRows(rows));
  const left = $derived(conflictCount(regions));
  const auto = $derived(autoResolvedCount(regions));
  const parsed = $derived(syntacticCount(regions));
  const current = $derived(at === null ? 0 : conflicts.indexOf(at) + 1);

  /** The three columns move sideways together, as the two halves of a diff do (R-470). */
  const PROBE = "0".repeat(100);
  let cellWidth = $state(0);
  let probeWidth = $state(0);
  let sideways = $state(0);
  const widest = $derived.by(() => {
    let most = 0;
    for (const row of rows) {
      for (const text of [row.ours, row.result, row.theirs]) {
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

  function step(direction: 1 | -1) {
    at = nextConflict(conflicts, at, direction);
  }

  /** In the capture phase, ahead of the main window's panel walk, as the diff's F6 is. */
  function onpanelkey(event: KeyboardEvent) {
    if (!active || modals.any) return;
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

<svelte:window onkeydowncapture={onpanelkey} />

<div class="merge">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    <span class="count" class:clean={left === 0}>{conflictsLeftLabel(left)}</span>
    {#if auto > 0}
      <span class="auto" title="Taken without asking; look before you commit">
        {auto} resolved automatically{parsed > 0 ? `, ${parsed} by the parser` : ""} — worth a look
      </span>
    {/if}
    <span class="grow"></span>
    <span class="nav">
      <button type="button" onclick={() => step(-1)} disabled={conflicts.length === 0} title="Previous conflict (F6 / Shift+F6)">↑</button>
      <span class="tabular">{current}/{conflicts.length}</span>
      <button type="button" onclick={() => step(1)} disabled={conflicts.length === 0} title="Next conflict">↓</button>
    </span>
    <button type="button" onclick={() => onresolve("ours")} title="Take the whole file as Ours has it">Take ours</button>
    <button type="button" onclick={() => onresolve("theirs")} title="Take the whole file as Theirs has it">Take theirs</button>
    <button type="button" class="primary" onclick={onsolver} title="Open the Conflict Solver for this file">Resolve…</button>
    <button type="button" onclick={oncancel}>Close</button>
  </div>

  <div class="heads">
    <span>Ours</span>
    <span>Result</span>
    <span>Theirs</span>
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="panes" style:--shift="{shift}px" {onwheel}>
    <div class="line ruler" aria-hidden="true">
      <span class="cell mono" bind:clientWidth={cellWidth}
        ><span class="probe" bind:offsetWidth={probeWidth}>{PROBE}</span></span
      >
    </div>
    <VirtualList items={rows} reveal={at} label="Merge preview">
      {#snippet row(entry, index)}
        <div
          class="line"
          class:conflict={entry.conflict}
          class:auto={entry.origin !== null && entry.origin !== "unchanged"}
          class:here={at !== null && entry.region === rows[at]?.region}
          style:top="{index * 22}px"
        >
          <span class="cell mono"><span class="text">{entry.ours ?? ""}</span></span>
          <span class="cell mono"><span class="text">{entry.result ?? ""}</span></span>
          <span class="cell mono"><span class="text">{entry.theirs ?? ""}</span></span>
        </div>
      {/snippet}
    </VirtualList>
  </div>
  <SidewaysScrollbar offset={shift} max={sidewaysMax} onscroll={(offset) => (sideways = offset)} />
</div>

<style>
  .merge {
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
    padding: var(--sp-3) var(--sp-5);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
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
    color: var(--fg-on-accent);
    border-color: var(--status-ref);
  }

  button:disabled {
    opacity: 0.5;
  }

  .heads {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    flex: 0 0 auto;
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .heads span {
    padding-left: var(--sp-3);
  }

  .line {
    position: absolute;
    left: 0;
    right: 0;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-code);
    line-height: 22px;
  }

  .line.conflict {
    background: var(--diff-changed-line);
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

  /* Column separators: the same divider as the bars, on the heads and on every row. */
  .cell + .cell,
  .heads span + span {
    border-left: 1px solid var(--divider);
  }

  .text {
    padding-left: var(--sp-3);
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
</style>
