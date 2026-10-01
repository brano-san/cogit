<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Button from "$components/common/Button.svelte";
  import CommandOutput from "$components/layout/CommandOutput.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { installChildWindow } from "$lib/child-window";
  import { repoNameOf } from "$lib/error-window";
  import {
    closeThisWindow,
    commandOutcome,
    focusMainWindow,
    getAppInfo,
    onErrorQueue,
    sendErrorsAction,
    type ErrorEntry,
    type GitOutput,
  } from "$lib/ipc";
  import { followSettings } from "$lib/settings-sync";
  import { settings } from "$stores/settings.svelte";

  /** Every failed git command and every one that stopped on conflicts, with the whole output
      (doc/12-risks.md, R-639). The main window owns the queue and sends it on each change. */
  $effect(() => installChildWindow(window));
  $effect(() => followSettings(() => void settings.reload()));
  $effect(() => void settings.load());

  let entries = $state.raw<ErrorEntry[]>([]);
  let selected = $state<number | null>(null);
  let record = $state.raw<GitOutput | null>(null);
  let logPath = $state("");

  const current = $derived(entries.find((entry) => entry.id === selected) ?? entries[0]);

  $effect(() => {
    void getAppInfo()
      .then((info) => (logPath = info.logPath))
      .catch(() => {});
  });

  $effect(() => {
    const pending = onErrorQueue((next) => {
      entries = next;
      if (next.length === 0) void closeThisWindow();
    });
    void pending.then(() => sendErrorsAction("ready")).catch(() => {});
    return () => void pending.then((stop) => stop()).catch(() => {});
  });

  // The window closing by any road — ✕, Esc, Ctrl+W, the title bar — tells the main window.
  $effect(() => {
    const pending = getCurrentWindow().onCloseRequested(async () => {
      await sendErrorsAction("closed").catch(() => {});
    });
    return () => void pending.then((stop) => stop()).catch(() => {});
  });

  $effect(() => {
    const entry = current;
    if (!entry) return;
    const id = entry.id;
    void sendErrorsAction("viewed", id).catch(() => {});
    void commandOutcome(id)
      .catch(() => null)
      .then((found) => {
        if (current?.id === id) record = found ?? summary(entry);
      });
  });

  /** The record rotated out of the journal: what the entry knows, said as it is. */
  function summary(entry: ErrorEntry): GitOutput {
    return {
      id: entry.id,
      repo: entry.repo,
      command: entry.command,
      exitCode: null,
      stdout: "",
      stderr: entry.summary,
      durationMs: 0,
      operation: entry.operation,
      severity: entry.kind === "error" ? "failure" : "warning",
      summary: entry.summary,
      startedAtMs: Date.now(),
      stoppedOnConflicts: entry.kind === "warning",
    };
  }

  async function showConflicts(entry: ErrorEntry) {
    await sendErrorsAction("showConflicts", entry.id);
    await focusMainWindow().catch(() => {});
  }

  function dismiss(entry: ErrorEntry) {
    void sendErrorsAction("dismiss", entry.id);
  }
</script>

<main class="errors">
  {#if entries.length > 1}
    <nav aria-label="Errors">
      {#each entries as entry (entry.id)}
        <button
          type="button"
          class="row {entry.kind}"
          class:on={entry.id === current?.id}
          aria-current={entry.id === current?.id}
          title={entry.summary}
          onclick={() => (selected = entry.id)}
        >
          <span class="dot" aria-hidden="true"></span>
          <span class="text">
            <span class="name truncate">{entry.title}</span>
            <span class="where truncate">{repoNameOf(entry.repo)}</span>
          </span>
          {#if entry.repeats > 1}<span class="repeats tabular">×{entry.repeats}</span>{/if}
        </button>
      {/each}
    </nav>
  {/if}

  <div class="detail">
    {#if current && record && record.id === current.id}
      <CommandOutput
        docked
        entry={record}
        {logPath}
        heading={current.title}
        warned={current.kind === "warning"}
        onclose={() => void closeThisWindow()}
      >
        {#snippet actions()}
          {#if current.kind === "warning"}
            <Button variant="primary" onclick={() => void showConflicts(current)}>Show conflicts</Button>
          {/if}
          {#if entries.length > 1}
            <Button onclick={() => dismiss(current)} title="Remove this entry from the list">Dismiss</Button>
          {/if}
        {/snippet}
      </CommandOutput>
    {/if}
  </div>
</main>

<TooltipLayer />

<style>
  .errors {
    display: flex;
    height: 100%;
    background: var(--surface-base);
    color: var(--text-primary);
    font-size: var(--fs-dense);
  }

  nav {
    display: flex;
    flex: none;
    flex-direction: column;
    width: 240px;
    overflow-y: auto;
    background: var(--surface-panel);
    border-right: 1px solid var(--divider);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    background: none;
    color: inherit;
    border: 0;
    border-bottom: 1px solid var(--divider);
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .row:hover {
    background: var(--surface-raised);
  }

  .row.on {
    background: var(--state-selected);
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--status-delete);
  }

  .warning .dot {
    background: var(--status-modify);
  }

  .text {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
  }

  .name {
    font-weight: 600;
  }

  .where {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .repeats {
    flex: none;
    color: var(--text-secondary);
  }

  .detail {
    flex: 1 1 auto;
    min-width: 0;
  }
</style>
