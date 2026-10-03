<script lang="ts">
  import type { SolverData } from "$lib/ipc";
  import { externalToolRefusal, submoduleCommit, wholeReason } from "$lib/conflict-kind";

  /** A file that cannot be merged line by line: binary, not UTF-8, or too large. Only a side
      taken whole, or an external tool, can settle it. */
  interface Props {
    data: SolverData;
    path: string;
    locked: boolean;
    onkeep: (side: "ours" | "theirs") => void;
    ondelete: () => void;
    onexternal: () => void;
  }

  let { data, path, locked, onkeep, ondelete, onexternal }: Props = $props();

  const why = $derived(wholeReason(data.kind, data.binary));
  const submodule = $derived(data.kind === "submodule");
  const keep = (side: "ours" | "theirs") =>
    submodule
      ? `Submodule: keep ${side} (${submoduleCommit(data.stages, side) ?? "none"})`
      : `Keep file (${side})`;
</script>

<div class="whole">
  <p class="what mono">{path}</p>
  <p>{why}</p>
  <p class="detail">
    {submodule
      ? "Keep the commit one side points to."
      : "Keep one version of the file whole, or open it in an external merge tool."}
  </p>
  <div class="actions">
    <button type="button" disabled={locked || data.missingOurs} onclick={() => onkeep("ours")}>
      {keep("ours")}
    </button>
    <button type="button" disabled={locked || data.missingTheirs} onclick={() => onkeep("theirs")}>
      {keep("theirs")}
    </button>
    {#if data.missingOurs || data.missingTheirs}
      <button type="button" class="danger" disabled={locked} onclick={ondelete}>Delete file</button>
    {/if}
    <button
      type="button"
      disabled={locked || submodule}
      title={externalToolRefusal(data.kind)}
      onclick={onexternal}>Open in external tool</button
    >
  </div>
  {#if data.missingOurs}<p class="detail">Deleted in ours.</p>{/if}
  {#if data.missingTheirs}<p class="detail">Deleted in theirs.</p>{/if}
</div>

<style>
  .whole {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-4);
    padding: var(--sp-7);
    font-size: var(--fs-dense);
    color: var(--fg-primary);
  }

  p {
    margin: 0;
  }

  .what {
    color: var(--fg-secondary);
    user-select: text;
  }

  .detail {
    color: var(--fg-secondary);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }

  button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
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

  button:disabled {
    opacity: 0.4;
  }

  button.danger {
    border-color: var(--status-danger);
    color: var(--status-danger);
  }
</style>
