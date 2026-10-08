<script lang="ts">
  import EmptyState from "$components/common/EmptyState.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import { CopyFeedback } from "$lib/copy-feedback.svelte";
  import { commandLogText, type CommandRow } from "$lib/ipc";
  import { isFailure, output } from "$stores/output.svelte";

  const clock = new Intl.DateTimeFormat(undefined, { timeStyle: "medium" });

  function repoName(entry: CommandRow): string {
    return entry.repo.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? entry.repo;
  }

  const feedback = new CopyFeedback();
  let panel: HTMLElement | undefined = $state();

  /** Only an Esc pressed inside it: one that closes a dialog or a search is not for it. */
  function onkeydown(event: KeyboardEvent) {
    const inside = panel !== undefined && event.target instanceof Node && panel.contains(event.target);
    if (event.key === "Escape" && output.open && inside && !event.defaultPrevented) output.open = false;
  }
</script>

<svelte:window {onkeydown} />

<div class="output" bind:this={panel} tabindex="-1">
  <header>
    <span class="title">Output</span>
    <span class="problems"><Checkbox bind:checked={output.errorsOnly} label="Problems only" /></span>
    <span class="grow"></span>
    <button type="button" class="btn sm" onclick={() => void feedback.copy(commandLogText(output.errorsOnly))}>
      {feedback.label("Copy log")}
    </button>
    <button type="button" class="btn sm" onclick={() => void output.clear()}>Clear</button>
    <button type="button" class="btn sm" onclick={() => (output.open = false)} title="Close (Esc)">✕</button>
  </header>

  {#if output.shownEntries.length === 0}
    <EmptyState title="No Git command has run yet." />
  {:else}
    <div class="list">
      {#each output.shownEntries as entry (entry.id)}
        <div class="entry" class:failed={isFailure(entry)} class:warned={entry.warned}>
          <button
            type="button"
            class="line"
            title="Show the output of this command"
            onclick={() => void output.openRecord(entry.id)}
          >
            <span class="when tabular">{clock.format(entry.startedAtMs)}</span>
            <span class="what">{entry.operation}</span>
            <span class="where truncate">{repoName(entry)}</span>
            <span class="cmd mono truncate">{entry.command}</span>
            <span class="meta tabular">{entry.exitCode ?? "?"} · {entry.durationMs} ms</span>
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .output {
    display: flex;
    flex-direction: column;
    flex: 0 0 auto;
    max-height: 32vh;
    border-top: 1px solid var(--divider);
    background: var(--surface-panel);
    font-size: var(--fs-dense);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: 0 0 auto;
    height: var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
  }

  .title {
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  .problems {
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    height: 22px;
    padding: 0 var(--sp-5);
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  .line:hover {
    background: var(--state-hover);
  }

  .when {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .what {
    flex: 0 0 auto;
    min-width: 72px;
  }

  .where {
    flex: 0 1 140px;
    min-width: 0;
    color: var(--text-secondary);
  }

  .cmd {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
  }

  .meta {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .entry.failed .what {
    color: var(--status-delete);
  }

  .entry.warned .what {
    color: var(--status-modify);
  }
</style>
