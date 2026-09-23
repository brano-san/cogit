<script lang="ts">
  import { deeperHint, describeCandidate, likelihoodLabel } from "$lib/investigate/origin";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";

  /** The card under the picked line: where its block came from and the way deeper. */
  interface Props {
    session: InvestigateSession;
    /** Who introduced the block, as blame named it: `1a2b3c4 · Alice · 140d`. */
    introduced: string;
  }

  let { session, introduced }: Props = $props();

  const blockPath = $derived(session.query?.previous?.path ?? session.query?.path ?? "");
  const words = $derived(session.candidate ? describeCandidate(session.candidate, blockPath) : null);
</script>

<!-- A click on the card must not reach the line under it and restart the search. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="card"
  role="dialog"
  tabindex="-1"
  aria-label="Origin of the selected lines"
  onclick={(event) => event.stopPropagation()}
>
  <div class="text">
    {#if session.search === "searching"}
      <span class="title">Searching for the origin…</span>
      <span class="detail">{introduced}</span>
    {:else if session.search === "failed"}
      <span class="title">The origin search failed</span>
      <span class="detail error">{session.searchError}</span>
    {:else if session.search === "cancelled"}
      <span class="title">The origin search was stopped</span>
    {:else if words && session.report}
      <span class="title">{words.title}</span>
      <span class="detail">{words.detail} — {introduced}</span>
      <span class="likelihood">{likelihoodLabel(session.report, session.chosen)}</span>
    {/if}
  </div>
  <button
    type="button"
    class="deeper"
    disabled={!session.candidate?.deeper}
    title="{deeperHint(session.candidate)} (Ctrl+D)"
    onclick={(event) => {
      event.stopPropagation();
      void session.goDeeper();
    }}>Go Deeper</button
  >
  <button
    type="button"
    class="close"
    aria-label="Hide the origin card"
    title="Hide the origin card"
    onclick={(event) => {
      event.stopPropagation();
      session.closeCard();
    }}>✕</button
  >
</div>

<style>
  .card {
    position: absolute;
    top: 100%;
    left: var(--card-left, 40px);
    z-index: 5;
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
    max-width: min(640px, 90%);
    padding: var(--sp-4) var(--sp-5);
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
    font-family: var(--font-ui);
    font-size: var(--fs-dense);
    white-space: normal;
    cursor: default;
  }

  .text {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }

  .title {
    font-weight: 600;
  }

  .detail {
    color: var(--text-secondary);
  }

  .error {
    color: var(--status-delete);
  }

  .likelihood {
    color: var(--status-ref);
  }

  button {
    flex: none;
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    cursor: default;
  }

  button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  button:disabled {
    color: var(--text-secondary);
    opacity: 0.55;
  }

  .close {
    padding: 0 var(--sp-3);
    background: none;
    border-color: transparent;
  }
</style>
