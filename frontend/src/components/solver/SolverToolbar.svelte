<script lang="ts">
  import { prettyKeys } from "$lib/keymap";
  import Icon from "$components/common/Icon.svelte";
  import type { GlyphName } from "$lib/icons";
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
    toolBlock: string | null;
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
    toolBlock,
    saving,
  }: Props = $props();

  const LAYOUT_ICON: Record<SolverLayout, GlyphName> = {
    oursResult: "layoutOursResult",
    all: "layoutAll",
    resultTheirs: "layoutResultTheirs",
    resultBelow: "layoutResultBelow",
  };
</script>

<div class="bar" role="toolbar" aria-label="Conflict Solver">
  <button
    type="button"
    class="tool"
    aria-pressed={baseChanges}
    disabled={locked}
    title="Color Ours and Theirs by what each changed in the base; off: by how the Result differs from them"
    onclick={() => onbase(!baseChanges)}><Icon name="base" /><span>Base Changes</span></button
  >
  <button
    type="button"
    class="tool"
    disabled={locked || saving || !dirty}
    title="Write the Result to the working file; it stays conflicted until Mark Resolved ({prettyKeys('CmdOrCtrl+S')})"
    onclick={onsave}><Icon name="save" /><span>Save</span></button
  >
  <button type="button" class="tool" disabled={locked || !hasChanges} title="Previous change ({prettyKeys('Shift+F6')})" onclick={() => onstep("change", -1)}
    ><Icon name="prev" /><span>Prev. Change</span></button
  >
  <button type="button" class="tool" disabled={locked || !hasChanges} title="Next change (F6)" onclick={() => onstep("change", 1)}
    ><Icon name="next" /><span>Next Change</span></button
  >
  <span class="sep" aria-hidden="true"></span>
  <div class="group" role="group" aria-label="Take">
    <button type="button" class="tool" disabled={locked || !can?.oursTheirs} title="Ours, then Theirs, for the current change ({prettyKeys('CmdOrCtrl+3')})" onclick={() => ontake("oursTheirs")}
      ><Icon name="takeOursTheirs" /><span>Take Ours, Theirs</span></button
    >
    <button type="button" class="tool" disabled={locked || !can?.ours} title="Take Ours for the current change ({prettyKeys('CmdOrCtrl+1')})" onclick={() => ontake("ours")}
      ><Icon name="takeOurs" /><span>Take Ours</span></button
    >
    <button type="button" class="tool" disabled={locked || !can?.theirs} title="Take Theirs for the current change ({prettyKeys('CmdOrCtrl+2')})" onclick={() => ontake("theirs")}
      ><Icon name="takeTheirs" /><span>Take Theirs</span></button
    >
    <button type="button" class="tool" disabled={locked || !can?.theirsOurs} title="Theirs, then Ours, for the current change ({prettyKeys('CmdOrCtrl+4')})" onclick={() => ontake("theirsOurs")}
      ><Icon name="takeTheirsOurs" /><span>Take Theirs, Ours</span></button
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
        onclick={() => onlayout(entry.id)}><Icon name={LAYOUT_ICON[entry.id]} /><span>{entry.label}</span></button
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
        onclick={() => onkeep("ours")}><Icon name="keepOurs" /><span>Keep Ours</span></button
      >
      <button
        type="button"
        class="tool"
        disabled={locked || deleted.theirs}
        title={deleted.theirs ? "Theirs deleted this file" : "Keep the file as Theirs has it, and stage it"}
        onclick={() => onkeep("theirs")}><Icon name="keepTheirs" /><span>Keep Theirs</span></button
      >
      <button type="button" class="tool danger" disabled={locked} title="Delete the file and stage the deletion" onclick={ondelete}
        ><Icon name="delete" /><span>Delete File</span></button
      >
    </div>
  {/if}
  <span class="grow"></span>
  <button type="button" class="tool" disabled={locked || !hasConflicts} title="Previous conflict ({prettyKeys('Shift+F7')})" onclick={() => onstep("conflict", -1)}
    ><Icon name="prevConflict" /><span>Prev. Conflict</span></button
  >
  <button type="button" class="tool" disabled={locked || !hasConflicts} title="Next conflict (F7)" onclick={() => onstep("conflict", 1)}
    ><Icon name="nextConflict" /><span>Next Conflict</span></button
  >
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="tool" disabled={locked || !!toolBlock} title={toolBlock ?? "Run the merge tool set in Preferences or in the Git config on this file"} onclick={onexternal}
    ><Icon name="external" /><span>External Tool</span></button
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

  .tool :global(svg) {
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
