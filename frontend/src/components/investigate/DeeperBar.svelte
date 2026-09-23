<script lang="ts">
  import { deeperHint } from "$lib/investigate/origin";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";

  /** The strip along the right edge: Go Deeper, with the state of the search on it. */
  interface Props {
    session: InvestigateSession;
  }

  let { session }: Props = $props();

  const STATE_LABEL = {
    idle: "No search running",
    searching: "Searching for the origin…",
    done: "Origin found",
    cancelled: "Search stopped",
    failed: "Search failed",
  } as const;

  const ready = $derived(session.search === "done" && !!session.candidate?.deeper);
</script>

<button
  type="button"
  class="bar {session.search}"
  disabled={!ready}
  title="{STATE_LABEL[session.search]}. {deeperHint(session.candidate)} (Ctrl+D)"
  aria-label="Blame (go deeper)"
  onclick={() => void session.goDeeper()}
>
  <span class="state" aria-hidden="true"></span>
  <span class="label">Blame (go deeper)</span>
</button>

<style>
  .bar {
    display: flex;
    flex: 0 0 24px;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) 0;
    background: var(--surface-panel);
    color: var(--text-primary);
    border: 0;
    border-left: 1px solid var(--divider);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  .bar:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .bar:disabled {
    color: var(--text-secondary);
  }

  .label {
    writing-mode: vertical-rl;
    white-space: nowrap;
  }

  .state {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-secondary);
  }

  .searching .state {
    background: var(--status-modify);
    animation: pulse 0.9s ease-in-out infinite alternate;
  }

  .done .state {
    background: var(--status-add);
  }

  .failed .state,
  .cancelled .state {
    background: var(--status-delete);
  }

  @keyframes pulse {
    from {
      opacity: 0.25;
    }
    to {
      opacity: 1;
    }
  }
</style>
