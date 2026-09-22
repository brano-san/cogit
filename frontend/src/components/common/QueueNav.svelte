<script lang="ts">
  /** `‹ 2 of 5 ›` for any queue the user steps through: failures, repository warnings. */
  interface Props {
    at: number;
    total: number;
    /** What one entry is called, for the button titles. */
    noun: string;
    onstep: (delta: -1 | 1) => void;
  }

  let { at, total, noun, onstep }: Props = $props();
</script>

{#if total > 1}
  <button
    type="button"
    class="step"
    disabled={at === 0}
    title="Previous {noun}"
    onclick={() => onstep(-1)}>‹</button
  >
  <span class="count tabular">{at + 1} of {total}</span>
  <button
    type="button"
    class="step"
    disabled={at === total - 1}
    title="Next {noun}"
    onclick={() => onstep(1)}>›</button
  >
{/if}

<style>
  .step {
    min-width: 22px;
    height: var(--h-button-sm);
    padding: 0 var(--sp-2);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .step:hover:not(:disabled) {
    border-color: var(--status-ref);
  }

  .step:disabled {
    opacity: 0.5;
  }

  .count {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }
</style>
