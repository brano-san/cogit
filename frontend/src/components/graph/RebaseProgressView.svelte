<script lang="ts">
  import type { HookRun, RebaseProgress } from "$lib/ipc";

  interface Props {
    progress: RebaseProgress;
    changes: number;
    staged: number;
    /** What to run at every pause; empty means the user has not asked for a check. */
    check: string;
    oncheck: (command: string) => void;
    onrun: () => void;
    /** The last verdict, or null before anything ran. */
    verdict: HookRun | null;
    running: boolean;
  }

  let { progress, changes, staged, check, oncheck, onrun, verdict, running }: Props = $props();

  const failed = $derived(verdict !== null && verdict.exitCode !== 0);
</script>

<div class="stack" aria-label="Rebase in progress">
  <div class="row virtual">
    <span class="node" aria-hidden="true">●</span>
    <span class="label">Working Tree</span>
    <span class="count">{changes} changes</span>
  </div>

  <div class="row virtual">
    <span class="node" aria-hidden="true">●</span>
    <span class="label">Index</span>
    <span class="count">{staged} changes</span>
    {#if progress.applying}
      <span class="applying truncate">rebasing: {progress.applying}</span>
    {/if}
  </div>

  {#each progress.todo as step (step.oid)}
    <div class="row todo">
      <span class="node" aria-hidden="true">◌</span>
      <span class="badge">{step.action}</span>
      <span class="label truncate">{step.summary || step.oid.slice(0, 7)}</span>
    </div>
  {/each}

  <div class="check" class:failed>
    <input
      type="text"
      value={check}
      placeholder="Check command, e.g. cargo test"
      aria-label="Command to run at every pause"
      oninput={(event) => oncheck(event.currentTarget.value)}
    />
    <button type="button" disabled={check.trim() === "" || running} onclick={onrun}>
      {running ? "Running…" : "Run"}
    </button>
    {#if verdict}
      <span class="verdict" title={verdict.stderr || verdict.stdout}>
        {failed ? `failed (${verdict.exitCode ?? "no code"})` : "passed"} ·
        {verdict.durationMs} ms
      </span>
    {/if}
  </div>
  {#if failed}
    <p class="note">
      The check failed. Nothing was aborted — continue, fix the step, or abort yourself.
    </p>
  {/if}

  <div class="row onto">
    <span class="node" aria-hidden="true">▶</span>
    <span class="label">
      Replaying onto {progress.onto ? progress.onto.slice(0, 7) : "the new base"}
    </span>
    <span class="count">{progress.done} of {progress.total} done</span>
  </div>
</div>

<style>
  .check {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
    border-top: 1px solid var(--divider);
  }

  .check input {
    flex: 1 1 auto;
    min-width: 0;
    height: var(--h-input);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
  }

  .check.failed input {
    border-color: var(--status-delete);
  }

  .verdict {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .check.failed .verdict {
    color: var(--status-delete);
  }

  .note {
    margin: 0;
    padding: 0 var(--sp-5) var(--sp-3);
    color: var(--status-modify);
    font-size: 11px;
  }

  .stack {
    flex: 0 0 auto;
    padding: var(--sp-2) 0;
    background: var(--c-modified-bg);
    border-bottom: 1px solid var(--divider);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
  }

  .node {
    flex: 0 0 12px;
    color: var(--status-modify);
    text-align: center;
  }

  .row.todo {
    color: var(--text-secondary);
  }

  .row.todo .node {
    color: var(--text-secondary);
  }

  .row.onto .node {
    color: var(--status-ref);
  }

  .label {
    min-width: 0;
  }

  .count,
  .applying {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .applying {
    flex: 1 1 auto;
    min-width: 0;
  }

  .badge {
    flex: 0 0 auto;
    padding: 0 var(--sp-2);
    background: var(--surface-raised);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: 10px;
  }
</style>
