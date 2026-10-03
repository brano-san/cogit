<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Button from "$components/common/Button.svelte";
  import CommandOutput from "$components/layout/CommandOutput.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { installChildWindow } from "$lib/child-window";
  import { outputToShow, repoNameOf } from "$lib/error-window";
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
  import { revealOnDesktop } from "$lib/ipc/file-menus";
  import { asCogitError } from "$lib/notices";
  import { followSettings } from "$lib/settings-sync";
  import { settings } from "$stores/settings.svelte";

  /** Every failed git command and every one that stopped on conflicts, with the whole output
      (doc/12-risks.md, R-639). The main window owns the queue and sends it on each change. */
  $effect(() => installChildWindow(window));
  $effect(() => followSettings(() => void settings.reload()));
  $effect(() => void settings.load());

  let entries = $state.raw<ErrorEntry[]>([]);
  let selected = $state<number | null>(null);
  let record = $state.raw<{ id: number; shown: GitOutput | "gone" } | null>(null);
  let logPath = $state("");
  let logProblem = $state("");
  /** What the journal gave for an entry, kept while the entry is in the queue: the journal
      is a ring of 100 commands and may lose it before the user comes back. */
  const outputs = new Map<number, GitOutput>();

  const current = $derived(entries.find((entry) => entry.id === selected) ?? entries[0]);
  // Keyed by id: every queue update brings new entry objects, and asking again for an
  // output already received could only get a worse answer.
  const currentId = $derived(current?.id);

  $effect(() => {
    void getAppInfo()
      .then((info) => (logPath = info.logPath))
      .catch(() => {});
  });

  $effect(() => {
    const pending = onErrorQueue((next) => {
      entries = next;
      const alive = new Set(next.map((entry) => entry.id));
      for (const id of outputs.keys()) if (!alive.has(id)) outputs.delete(id);
      if (next.length === 0) void closeThisWindow();
    });
    void pending.then(() => sendErrorsAction("ready")).catch(() => {});
    return () => void pending.then((stop) => stop()).catch(() => {});
  });

  // The window closing by any road — ✕, Esc, Ctrl+W, the title bar — tells the main window.
  $effect(() => {
    const pending = getCurrentWindow().onCloseRequested(async () => {
      // Everything this window holds was in front of it; an entry still on its way to it is not.
      await Promise.all(entries.map((entry) => sendErrorsAction("viewed", entry.id).catch(() => {})));
      await sendErrorsAction("closed").catch(() => {});
    });
    return () => void pending.then((stop) => stop()).catch(() => {});
  });

  $effect(() => {
    const id = currentId;
    if (id === undefined) return;
    void sendErrorsAction("viewed", id).catch(() => {});
    const cached = outputs.get(id);
    if (cached) {
      record = { id, shown: cached };
      return;
    }
    void commandOutcome(id)
      .catch(() => null)
      .then((found) => {
        const shown = outputToShow(found, outputs.get(id));
        if (shown !== "gone") outputs.set(id, shown);
        if (currentId === id) record = { id, shown };
      });
  });

  async function openLog() {
    logProblem = "";
    await revealOnDesktop(logPath).catch((err) => {
      logProblem = asCogitError(err)?.message ?? String(err);
    });
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
      {@const output = record.shown}
      {#if output === "gone"}
        <section class="gone">
          <h2>{current.title}</h2>
          <dl class="facts">
            <dt>Repository</dt>
            <dd class="mono" title={current.repo}>{repoNameOf(current.repo)}</dd>
            <dt>Command</dt>
            <dd class="mono">{current.command}</dd>
          </dl>
          <p>{current.summary}</p>
          <p class="note">
            The full output of this command is no longer in the journal. Open the log file to see it.
          </p>
          {#if logProblem}<p class="note">{logProblem}</p>{/if}
          <div class="buttons">
            <Button onclick={() => void openLog()} title={logPath}>Open log</Button>
            {#if current.kind === "warning"}
              <Button variant="primary" onclick={() => void showConflicts(current)}>Show conflicts</Button>
            {/if}
            {#if entries.length > 1}
              <Button onclick={() => dismiss(current)} title="Remove this entry from the list">Dismiss</Button>
            {/if}
          </div>
        </section>
      {:else}
      <CommandOutput
        docked
        entry={output}
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

  .gone {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }

  .gone h2 {
    margin: 0;
    font-size: var(--fs-header);
  }

  .gone .facts {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--sp-2) var(--sp-4);
    margin: 0;
  }

  .gone dt {
    color: var(--text-secondary);
  }

  .gone dd {
    margin: 0;
  }

  .gone p {
    margin: 0;
    white-space: pre-wrap;
  }

  .gone .note {
    color: var(--text-secondary);
  }

  .gone .buttons {
    display: flex;
    gap: var(--sp-3);
  }
</style>
