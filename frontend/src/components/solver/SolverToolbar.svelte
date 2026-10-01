<script lang="ts">
  import { LAYOUTS, type SolverLayout } from "$lib/solver-geometry";
  import type { TakeAction } from "$lib/solver-model";

  interface Props {
    layout: SolverLayout;
    onlayout: (layout: SolverLayout) => void;
    baseChanges: boolean;
    onbase: (on: boolean) => void;
    /** The file is merged line by line: there are hunks to walk and take. */
    merged: boolean;
    hasChanges: boolean;
    hasConflicts: boolean;
    onstep: (kind: "change" | "conflict", by: 1 | -1) => void;
    conflictsLabel: string;
    /** Nothing is current and nothing is undecided: the take buttons have nothing to act on. */
    canTake: boolean;
    ontake: (action: TakeAction) => void;
    /** Modify/delete: which side lost the file, and what can be done about it. */
    deleted: { ours: boolean; theirs: boolean } | null;
    onkeep: (side: "ours" | "theirs") => void;
    ondelete: () => void;
    onexternal: () => void;
    onsave: () => void;
    /** An external tool has the file: nothing here acts until it is done. */
    locked: boolean;
    saving: boolean;
  }

  let {
    layout,
    onlayout,
    baseChanges,
    onbase,
    merged,
    hasChanges,
    hasConflicts,
    onstep,
    conflictsLabel,
    canTake,
    ontake,
    deleted,
    onkeep,
    ondelete,
    onexternal,
    onsave,
    locked,
    saving,
  }: Props = $props();
</script>

<div class="bar" role="toolbar" aria-label="Conflict Solver">
  {#if merged}
    <div class="group" role="group" aria-label="Layout">
      {#each LAYOUTS as entry (entry.id)}
        <button
          type="button"
          aria-pressed={layout === entry.id}
          disabled={locked}
          title="Show {entry.label}"
          onclick={() => onlayout(entry.id)}>{entry.label}</button
        >
      {/each}
    </div>
    <button
      type="button"
      aria-pressed={baseChanges}
      disabled={locked}
      title="Color Ours and Theirs by what each changed in the base; off: by where they differ from each other"
      onclick={() => onbase(!baseChanges)}>Base Changes</button
    >
    <div class="group" role="group" aria-label="Changes">
      <button type="button" disabled={locked || !hasChanges} title="Previous change (Shift+F6)" onclick={() => onstep("change", -1)}>↑ Change</button>
      <button type="button" disabled={locked || !hasChanges} title="Next change (F6)" onclick={() => onstep("change", 1)}>↓ Change</button>
    </div>
    <div class="group" role="group" aria-label="Conflicts">
      <button type="button" disabled={locked || !hasConflicts} title="Previous conflict (Shift+F7)" onclick={() => onstep("conflict", -1)}>↑ Conflict</button>
      <button type="button" disabled={locked || !hasConflicts} title="Next conflict (F7)" onclick={() => onstep("conflict", 1)}>↓ Conflict</button>
    </div>
    <span class="count" class:clean={!hasConflicts} role="status">{conflictsLabel}</span>
    <div class="group" role="group" aria-label="Take">
      <button type="button" disabled={locked || !canTake} title="Take Ours for the current change (Ctrl+1)" onclick={() => ontake("ours")}>Take Ours</button>
      <button type="button" disabled={locked || !canTake} title="Take Theirs for the current change (Ctrl+2)" onclick={() => ontake("theirs")}>Take Theirs</button>
      <button type="button" disabled={locked || !canTake} title="Ours, then Theirs (Ctrl+3)" onclick={() => ontake("oursTheirs")}>Ours + Theirs</button>
      <button type="button" disabled={locked || !canTake} title="Theirs, then Ours (Ctrl+4)" onclick={() => ontake("theirsOurs")}>Theirs + Ours</button>
    </div>
  {/if}
  {#if deleted}
    <div class="group" role="group" aria-label="File">
      <button
        type="button"
        disabled={locked || deleted.ours}
        title={deleted.ours ? "Ours deleted this file" : "Keep the file as Ours has it, and stage it"}
        onclick={() => onkeep("ours")}>Keep file (ours)</button
      >
      <button
        type="button"
        disabled={locked || deleted.theirs}
        title={deleted.theirs ? "Theirs deleted this file" : "Keep the file as Theirs has it, and stage it"}
        onclick={() => onkeep("theirs")}>Keep file (theirs)</button
      >
      <button type="button" class="danger" disabled={locked} title="Delete the file and stage the deletion" onclick={ondelete}>Delete file</button>
    </div>
  {/if}
  <span class="grow"></span>
  <button type="button" disabled={locked} title="Run the merge tool set in Preferences or in the Git config on this file" onclick={onexternal}>Open in external tool</button>
  {#if merged || deleted}
    <button
      type="button"
      class="primary"
      disabled={locked || saving}
      title="Write the Result and stage the file (Ctrl+S)"
      onclick={onsave}>Save</button
    >
  {/if}
</div>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    min-height: 32px;
    padding: var(--sp-2) var(--sp-4);
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-dense);
    user-select: none;
  }

  .group {
    display: flex;
    gap: var(--sp-1);
  }

  .grow {
    flex: 1 1 auto;
  }

  button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--bg-input);
    color: var(--fg-primary);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  button[aria-pressed="true"] {
    background: var(--state-pressed);
    color: var(--state-pressed-text);
  }

  button:disabled {
    opacity: 0.4;
  }

  button.primary {
    background: var(--status-ref);
    border-color: var(--status-ref);
    color: var(--fg-on-accent);
  }

  button.danger {
    border-color: var(--status-danger);
    color: var(--status-danger);
  }

  .count {
    color: var(--status-danger);
    white-space: nowrap;
  }

  .count.clean {
    color: var(--status-success);
  }
</style>
