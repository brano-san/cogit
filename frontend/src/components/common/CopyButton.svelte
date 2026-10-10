<script lang="ts">
  import { CopyFeedback } from "$lib/copy-feedback.svelte";

  /** A copy button that answers on itself: a check mark and a "Copied!" tooltip for 1.5 s
      (`CopyFeedback`), "Copy failed" when the clipboard refuses. `icon`: two sheets instead of
      the label, which becomes the tooltip. */
  let {
    text,
    label = "Copy",
    icon = false,
    class: cls = "",
  }: { text: string; label?: string; icon?: boolean; class?: string } = $props();

  const feedback = new CopyFeedback();
</script>

<span class="wrap">
  <button type="button" class={cls} aria-label={label} title={icon ? label : undefined} onclick={() => void feedback.copy(text)}>
    {#if feedback.state === "copied"}
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
        ><path d="M3 8.5 6.5 12 13 4.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg
      >
    {:else if icon}
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.3"
        ><rect x="5.5" y="5.5" width="8" height="8" rx="1.5" /><path d="M10.5 3.5V3A1.5 1.5 0 0 0 9 1.5H4A1.5 1.5 0 0 0 2.5 3v5A1.5 1.5 0 0 0 4 9.5h.5" /></svg
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
