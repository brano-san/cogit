<script lang="ts">
  import type { Gap } from "$lib/diff-fold";

  /** A fold of the diff (#16): one row of the same height as a line of code (R-626), so the
      anchors of the blocks after it stay on the row grid. Unified and the left pane carry the
      controls; the right pane's strip carries the actions of the hunk below, where its header
      used to put them. Drawn by the diff view's own rows and by the editable pane alike.
      A fold that hides nothing is only that strip, where a block has no fold above it. */
  interface Props {
    gap: Gap;
    hunk: number | null;
    role: "both" | "left" | "right";
    stageable: boolean;
    actions: { stage: boolean; unstage: boolean; discard: boolean };
    /** Why Stage, Unstage and Discard wait: unsaved edits. */
    frozen?: string | null;
    onopen: (how: "up" | "down" | "all", event: MouseEvent) => void;
    onstage: (reverse: boolean) => void;
    ondiscard: () => void;
  }

  let { gap, hunk, role, stageable, actions, frozen = null, onopen, onstage, ondiscard }: Props = $props();
</script>

<div class="fold" role="group" aria-label={gap.hidden > 0 ? `${gap.hidden} lines hidden` : "Block actions"}>
  {#if role !== "right"}
    {#if gap.up}
      <button
        type="button"
        class="arrow"
        title="Show 20 more lines above the change below (Ctrl+click: every hidden line)"
        onclick={(event) => onopen("up", event)}>▲</button
      >
    {/if}
    {#if gap.hidden > 0}
      <button
        type="button"
        class="count"
        title="Show all {gap.hidden} hidden lines (Ctrl+click: every hidden line in the file)"
        onclick={(event) => onopen("all", event)}>{gap.hidden} lines hidden · show</button
      >
    {/if}
    {#if gap.down}
      <button
        type="button"
        class="arrow"
        title="Show 20 more lines below the change above (Ctrl+click: every hidden line)"
        onclick={(event) => onopen("down", event)}>▼</button
      >
    {/if}
    {#if gap.context}<span class="where truncate">{gap.context}</span>{/if}
  {/if}
  {#if role !== "left" && hunk !== null && stageable}
    <span class="acts">
      <button
        type="button"
        title={frozen ?? "Stage this block"}
        disabled={!actions.stage || frozen !== null}
        onclick={() => onstage(false)}>Stage</button
      >
      <button
        type="button"
        title={frozen ?? "Unstage this block"}
        disabled={!actions.unstage || frozen !== null}
        onclick={() => onstage(true)}>Unstage</button
      >
      {#if actions.discard}
        <button
          type="button"
          class="danger"
          title={frozen ?? "Throw this block away (always asks first)"}
          disabled={frozen !== null}
          onclick={ondiscard}>Discard</button
        >
      {/if}
    </span>
  {/if}
</div>

<style>
  /* One band in each pane, saying what is hidden (#16). */
  .fold {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 1 1 auto;
    min-width: 0;
    padding-left: var(--sp-4);
    background: var(--diff-hunk-header-bg);
    box-shadow:
      inset 0 1px 0 var(--border),
      inset 0 -1px 0 var(--border);
    color: var(--diff-hunk-header-fg);
    font-family: var(--font-ui);
    font-size: var(--fs-header);
    white-space: pre;
  }

  .fold > button {
    flex: 0 0 auto;
    height: 14px;
    padding: 0 var(--sp-2);
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--diff-hunk-header-fg);
    font: inherit;
    line-height: 14px;
    cursor: default;
  }

  .fold > button:hover {
    background: var(--bg-hover);
    color: var(--fg-primary);
  }

  .arrow {
    font-size: 9px;
  }

  .where {
    min-width: 0;
    margin-left: var(--sp-3);
    color: var(--diff-hunk-header-fg);
    font-family: var(--font-mono);
  }

  .acts {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
    margin-left: auto;
    padding-right: var(--sp-3);
  }

  .acts button {
    height: 14px;
    padding: 0 var(--sp-2);
    background: var(--bg-input);
    color: var(--fg-secondary);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
    font-size: 9px;
    line-height: 12px;
    cursor: default;
  }

  .acts button:hover:not(:disabled) {
    color: var(--fg-primary);
  }

  /* 06 §6: a control that cannot act says so, and does not light up under the pointer. */
  .acts button:disabled {
    opacity: 0.4;
  }

  .acts button.danger {
    color: var(--status-danger);
  }

  .acts button.danger:hover:not(:disabled) {
    border-color: var(--status-danger);
  }
</style>
