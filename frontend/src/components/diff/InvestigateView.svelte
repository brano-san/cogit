<script lang="ts">
  import { investigate, type InvestigationStep, type RepoId } from "$lib/ipc";

  interface Props {
    repo: RepoId;
    path: string;
    from: number;
    to: number;
    onclose: () => void;
    /** Opening a commit is the panel's job, not this view's. */
    onselectcommit?: (oid: string) => void;
  }

  let { repo, path, from, to, onclose, onselectcommit }: Props = $props();

  const LIMIT = 200;

  let steps = $state.raw<InvestigationStep[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let at = $state(0);

  const current = $derived(steps[at] ?? null);

  $effect(() => {
    const query = { repo, path, from, to };
    let live = true;
    loading = true;
    error = null;

    investigate(query.repo, query.path, query.from, query.to, LIMIT)
      .then((found) => {
        if (!live) return;
        steps = found;
        at = 0;
      })
      .catch((err: unknown) => {
        if (!live) return;
        steps = [];
        error = err instanceof Error ? err.message : String(err);
      })
      .finally(() => {
        if (live) loading = false;
      });

    return () => {
      live = false;
    };
  });

  function when(timestamp: number): string {
    return new Date(timestamp * 1000).toISOString().slice(0, 10);
  }
</script>

<div class="investigate">
  <div class="bar">
    <span class="mono truncate">{path}</span>
    <span class="range tabular">lines {from}–{to}</span>
    <span class="grow"></span>
    {#if !loading && steps.length > 0}
      <span class="count tabular">{steps.length} edits</span>
    {/if}
    <button type="button" title="Back to the diff" onclick={onclose}>✕</button>
  </div>

  {#if loading}
    <p class="message">Tracing the fragment…</p>
  {:else if error}
    <p class="message warn">{error}</p>
  {:else if steps.length === 0}
    <p class="message">Nothing has touched these lines.</p>
  {:else}
    <div class="body">
      <ul class="steps">
        {#each steps as step, index (step.oid + index)}
          <li>
            <button
              type="button"
              class="step"
              class:selected={index === at}
              onclick={() => (at = index)}
              ondblclick={() => onselectcommit?.(step.oid)}
            >
              <span class="oid mono">{step.oid.slice(0, 7)}</span>
              <span class="summary truncate">{step.summary}</span>
              <span class="who truncate">{step.author}</span>
              <span class="when tabular">{when(step.timestamp)}</span>
              {#if step.path !== path}
                <span class="renamed mono truncate" title="The file was called this then"
                  >{step.path}</span
                >
              {/if}
            </button>
          </li>
        {/each}
      </ul>

      {#if current}
        <pre class="patch mono">{current.diff}</pre>
      {/if}
    </div>
  {/if}
</div>

<style>
  .investigate {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .bar button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    cursor: default;
  }

  .grow {
    flex: 1 1 auto;
  }

  .range,
  .count {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
  }

  .steps {
    flex: 0 0 40%;
    min-height: 0;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
    border-bottom: 1px solid var(--divider);
  }

  .step {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    padding: 2px var(--sp-4);
    background: none;
    color: var(--text-primary);
    border: 0;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  .step:hover {
    background: var(--state-hover);
  }

  .step.selected {
    background: var(--state-selected, rgb(255 255 255 / 10%));
  }

  .oid {
    flex: 0 0 auto;
    color: var(--status-ref);
    font-size: var(--fs-code);
  }

  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  .who,
  .when {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .who {
    max-width: 140px;
  }

  /* The path the file had at that commit, shown only where a rename changed it. */
  .renamed {
    flex: 0 0 auto;
    max-width: 180px;
    color: var(--status-modify);
    font-size: 10px;
  }

  .patch {
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    overflow: auto;
    font-size: var(--fs-code);
    white-space: pre;
    user-select: text;
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .message.warn {
    color: var(--status-modify);
  }
</style>
