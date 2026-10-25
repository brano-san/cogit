<script lang="ts">
  import { LAYOUTS, type SolverLayout } from "$lib/solver-geometry";
  import type { TakeAction } from "$lib/solver-model";

  interface Props {
    layout: SolverLayout;
    onlayout: (layout: SolverLayout) => void;
    baseChanges: boolean;
    onbase: (on: boolean) => void;
    hasChanges: boolean;
    hasConflicts: boolean;
    onstep: (kind: "change" | "conflict", by: 1 | -1) => void;
    /** Which Take actions would change the target hunk; null when there is none. */
    can: Record<TakeAction, boolean> | null;
    ontake: (action: TakeAction) => void;
    /** Modify/delete: which side lost the file, and what can be done about it. */
    deleted: { ours: boolean; theirs: boolean } | null;
    onkeep: (side: "ours" | "theirs") => void;
    ondelete: () => void;
    onexternal: () => void;
    onsave: () => void;
    /** Stage the file as resolved, then close the window. */
    onresolve: () => void;
    /** The Result differs from the file on disk. */
    dirty?: boolean;
    /** An external tool has the file: nothing here acts until it is done. */
    locked: boolean;
    saving: boolean;
  }

  let {
    layout,
    onlayout,
    baseChanges,
    onbase,
    hasChanges,
    hasConflicts,
    onstep,
    can,
    ontake,
    deleted,
    onkeep,
    ondelete,
    onexternal,
    onsave,
    onresolve,
    dirty = false,
    locked,
    saving,
  }: Props = $props();

  /** 16×16 strokes, drawn in the button's color. */
  const ICONS: Record<string, string> = {
    base: "M8 14V8M8 8 4 3M8 8l4-5M2.5 3h3M10.5 3h3",
    save: "M3 2.5h8l2.5 2.5v8.5h-10.5zM5.5 2.5v3h5v-3M5.5 13.5v-4h5v4",
    prevChange: "M8 13V3M4 7l4-4 4 4",
    nextChange: "M8 3v10M4 9l4 4 4-4",
    takeOursTheirs: "M2 4.5h9M8 2l3 2.5L8 7M2 11.5h9M8 9l3 2.5L8 14M14 2v12",
    takeOurs: "M2 8h10M8.5 4.5 12 8l-3.5 3.5M14 3v10",
    takeTheirs: "M14 8H4M7.5 4.5 4 8l3.5 3.5M2 3v10",
    takeTheirsOurs: "M14 4.5H5M8 2 5 4.5 8 7M14 11.5H5M8 9l-3 2.5L8 14M2 2v12",
    oursResult: "M2 3h12v10H2zM8 3v10M3.5 5.5h3M3.5 8h3",
    all: "M2 3h12v10H2zM6 3v10M10 3v10",
    resultTheirs: "M2 3h12v10H2zM8 3v10M9.5 5.5h3M9.5 8h3",
    resultBelow: "M2 3h12v10H2zM2 8h12M8 3v5",
    prevConflict: "M8 13V5M4.5 8.5 8 5l3.5 3.5M3 2.5h10",
    nextConflict: "M8 3v8M4.5 7.5 8 11l3.5-3.5M3 13.5h10",
    keepOurs: "M4 2h5l3 3v9H4zM9 2v3h3M6 9.5l1.5 1.5L10 8",
    keepTheirs: "M4 2h5l3 3v9H4zM9 2v3h3M6 9.5l1.5 1.5L10 8",
    delete: "M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 9h5.6l.7-9M7 7v4.5M9 7v4.5",
    external: "M9 2.5h4.5V7M13.5 2.5 8 8M11.5 9.5v4h-9v-9h4",
  };

  const LAYOUT_ICON: Record<SolverLayout, string> = {
    oursResult: "oursResult",
    all: "all",
    resultTheirs: "resultTheirs",
    resultBelow: "resultBelow",
  };
</script>

{#snippet icon(name: string)}
  <svg viewBox="0 0 16 16" aria-hidden="true"><path d={ICONS[name]} /></svg>
{/snippet}

<div class="bar" role="toolbar" aria-label="Conflict Solver">
  <button
    type="button"
    class="tool"
    aria-pressed={baseChanges}
    disabled={locked}
    title="Color Ours and Theirs by what each changed in the base; off: by how the Result differs from them"
    onclick={() => onbase(!baseChanges)}>{@render icon("base")}<span>Base Changes</span></button
  >
  <button
    type="button"
    class="tool"
    disabled={locked || saving || !dirty}
    title="Write the Result to the working file; it stays conflicted until Mark Resolved (Ctrl+S)"
    onclick={onsave}>{@render icon("save")}<span>Save</span></button
  >
  <button type="button" class="tool" disabled={locked || !hasChanges} title="Previous change (Shift+F6)" onclick={() => onstep("change", -1)}
    >{@render icon("prevChange")}<span>Prev. Change</span></button
  >
  <button type="button" class="tool" disabled={locked || !hasChanges} title="Next change (F6)" onclick={() => onstep("change", 1)}
    >{@render icon("nextChange")}<span>Next Change</span></button
  >
  <span class="sep" aria-hidden="true"></span>
  <div class="group" role="group" aria-label="Take">
    <button type="button" class="tool" disabled={locked || !can?.oursTheirs} title="Ours, then Theirs, for the current change (Ctrl+3)" onclick={() => ontake("oursTheirs")}
      >{@render icon("takeOursTheirs")}<span>Take Ours, Theirs</span></button
    >
    <button type="button" class="tool" disabled={locked || !can?.ours} title="Take Ours for the current change (Ctrl+1)" onclick={() => ontake("ours")}
      >{@render icon("takeOurs")}<span>Take Ours</span></button
    >
    <button type="button" class="tool" disabled={locked || !can?.theirs} title="Take Theirs for the current change (Ctrl+2)" onclick={() => ontake("theirs")}
      >{@render icon("takeTheirs")}<span>Take Theirs</span></button
    >
    <button type="button" class="tool" disabled={locked || !can?.theirsOurs} title="Theirs, then Ours, for the current change (Ctrl+4)" onclick={() => ontake("theirsOurs")}
      >{@render icon("takeTheirsOurs")}<span>Take Theirs, Ours</span></button
    >
  </div>
  <span class="sep" aria-hidden="true"></span>
  <div class="group" role="group" aria-label="Layout">
    {#each LAYOUTS as entry (entry.id)}
      <button
        type="button"
        class="tool"
        aria-pressed={layout === entry.id}
        disabled={locked}
        title="Show {entry.label}"
        onclick={() => onlayout(entry.id)}>{@render icon(LAYOUT_ICON[entry.id])}<span>{entry.label}</span></button
      >
    {/each}
  </div>
  {#if deleted}
    <span class="sep" aria-hidden="true"></span>
    <div class="group" role="group" aria-label="File">
      <button
        type="button"
        class="tool"
        disabled={locked || deleted.ours}
        title={deleted.ours ? "Ours deleted this file" : "Keep the file as Ours has it, and stage it"}
        onclick={() => onkeep("ours")}>{@render icon("keepOurs")}<span>Keep Ours</span></button
      >
      <button
        type="button"
        class="tool"
        disabled={locked || deleted.theirs}
        title={deleted.theirs ? "Theirs deleted this file" : "Keep the file as Theirs has it, and stage it"}
        onclick={() => onkeep("theirs")}>{@render icon("keepTheirs")}<span>Keep Theirs</span></button
      >
      <button type="button" class="tool danger" disabled={locked} title="Delete the file and stage the deletion" onclick={ondelete}
        >{@render icon("delete")}<span>Delete File</span></button
      >
    </div>
  {/if}
  <span class="grow"></span>
  <button type="button" class="tool" disabled={locked || !hasConflicts} title="Previous conflict (Shift+F7)" onclick={() => onstep("conflict", -1)}
    >{@render icon("prevConflict")}<span>Prev. Conflict</span></button
  >
  <button type="button" class="tool" disabled={locked || !hasConflicts} title="Next conflict (F7)" onclick={() => onstep("conflict", 1)}
    >{@render icon("nextConflict")}<span>Next Conflict</span></button
  >
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="tool" disabled={locked} title="Run the merge tool set in Preferences or in the Git config on this file" onclick={onexternal}
    >{@render icon("external")}<span>External Tool</span></button
  >
  <button
    type="button"
    class="btn sm primary"
    disabled={locked || saving}
    title="Write the Result, stage the file as resolved and close"
    onclick={onresolve}>Mark Resolved</button
  >
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    flex: 0 0 auto;
    min-height: 48px;
    padding: var(--sp-1) var(--sp-4);
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-dense);
    flex-wrap: wrap;
    user-select: none;
  }

  .group {
    display: flex;
    gap: var(--sp-1);
  }

  .grow {
    flex: 1 1 auto;
  }

  .sep {
    flex: none;
    width: 1px;
    height: 32px;
    margin-inline: var(--sp-1);
    background: var(--divider);
  }

  /* Icon over its label, as SmartGit's toolbar has them. */
  .tool {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    flex: none;
    min-width: 36px;
    height: 40px;
    padding: 2px 3px;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    color: var(--fg-primary);
    font: inherit;
    font-size: 10px;
    line-height: 1.1;
    white-space: nowrap;
    cursor: default;
  }

  .tool svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .tool:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .tool[aria-pressed="true"] {
    background: var(--state-pressed);
    border-color: var(--border);
    color: var(--state-pressed-text);
  }

  .tool:disabled {
    color: var(--fg-disabled, var(--fg-secondary));
    opacity: 0.45;
  }

  .tool.danger:not(:disabled) {
    color: var(--status-danger);
  }

  .primary {
    margin-left: var(--sp-2);
  }
</style>
