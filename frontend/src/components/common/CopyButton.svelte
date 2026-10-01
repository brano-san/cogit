<script lang="ts">
  import { CopyFeedback } from "$lib/copy-feedback.svelte";

  /** A copy button that answers on itself: a check mark and a "Copied!" tooltip for 1.5 s
      (`CopyFeedback`), "Copy failed" when the clipboard refuses. */
  let { text, label = "Copy", class: cls = "" }: { text: string; label?: string; class?: string } = $props();

  const feedback = new CopyFeedback();
</script>

<span class="wrap">
  <button type="button" class={cls} aria-label={label} onclick={() => void feedback.copy(text)}>
    {#if feedback.state === "copied"}
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
        ><path d="M3 8.5 6.5 12 13 4.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg
      >
    {:else}{label}{/if}
  </button>
  {#if feedback.state !== "idle"}
    <span class="tip" class:failed={feedback.state === "failed"} role="status"
      >{feedback.state === "copied" ? "Copied!" : "Copy failed"}</span
    >
  {/if}
</span>

<style>
  .wrap {
    position: relative;
    display: inline-flex;
  }

  .tip {
    position: absolute;
    bottom: calc(100% + var(--sp-2));
    left: 50%;
    transform: translateX(-50%);
    padding: var(--sp-1) var(--sp-3);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: 1px solid var(--status-add);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    white-space: nowrap;
    pointer-events: none;
    box-shadow: var(--shadow-popover);
  }

  .tip.failed {
    border-color: var(--status-delete);
  }
</style>
