<script lang="ts">
  import type { HookRun } from "$lib/ipc";

  interface Props {
    /** What to run at every pause; empty means the user has not asked for a check. */
    check: string;
    oncheck: (command: string) => void;
    onrun: () => void;
    /** The last verdict, or null before anything ran. */
    verdict: HookRun | null;
    running: boolean;
  }

  let { check, oncheck, onrun, verdict, running }: Props = $props();

  const failed = $derived(verdict !== null && verdict.exitCode !== 0);
</script>

<div class="bar" aria-label="Rebase in progress">
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
</div>

<style>
  .bar {
    flex: 0 0 auto;
    background: var(--c-modified-bg);
    border-bottom: 1px solid var(--divider);
  }

  .check {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
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

  /* The same small button as the commit actions beside the graph. */
  .check button {
    height: 20px;
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .check button:hover:not(:disabled) {
    border-color: var(--status-ref);
  }

  .check button:disabled {
    opacity: 0.4;
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
</style>
