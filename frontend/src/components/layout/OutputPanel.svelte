<script lang="ts">
  import EmptyState from "$components/common/EmptyState.svelte";
  import { CopyFeedback } from "$lib/copy-feedback.svelte";
  import { commandLogText, type CommandRow } from "$lib/ipc";
  import { kindOf, output, type RowKind } from "$stores/output.svelte";

  let { height }: { height: number } = $props();

  const clock = new Intl.DateTimeFormat(undefined, { timeStyle: "medium" });

  function repoName(entry: CommandRow): string {
    return entry.repo.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? entry.repo;
  }

  const TOGGLES: readonly { kind: RowKind; label: string }[] = [
    { kind: "error", label: "Errors" },
    { kind: "warning", label: "Warnings" },
    { kind: "operation", label: "Operations" },
  ];

  const counts = $derived.by(() => {
    const found: Record<RowKind, number> = { error: 0, warning: 0, operation: 0 };
    for (const entry of output.entries) found[kindOf(entry)] += 1;
    return found;
  });

  function toggle(kind: RowKind) {
    output.showing = { ...output.showing, [kind]: !output.showing[kind] };
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

<div class="output" bind:this={panel} tabindex="-1" style:height="{height}px">
  <header>
    <span class="title">Output</span>
    {#each TOGGLES as toggleOf (toggleOf.kind)}
      <button
        type="button"
        class="btn sm kind-{toggleOf.kind}"
        aria-pressed={output.showing[toggleOf.kind]}
        onclick={() => toggle(toggleOf.kind)}><span class="tabular">{toggleOf.label} {counts[toggleOf.kind]}</span></button
      >
    {/each}
    <span class="grow"></span>
    <button
      type="button"
      class="btn sm"
      onclick={() => void feedback.copy(commandLogText(output.shownEntries.map((entry) => entry.id)))}
    >
      {feedback.label("Copy log")}
    </button>
    <button type="button" class="btn sm" onclick={() => void output.clear()}>Clear</button>
    <button type="button" class="btn sm" onclick={() => (output.open = false)} title="Close (Esc)">✕</button>
  </header>

  {#if output.entries.length === 0}
    <EmptyState title="No Git command has run yet." />
  {:else if output.shownEntries.length === 0}
    <EmptyState title="Nothing matches the shown kinds." />
  {:else}
    <div class="list">
      <div class="line head">
        <span>Time</span>
        <span>Operation</span>
        <span>Repository</span>
        <span>Command</span>
        <span class="num">Exit</span>
        <span class="num">Duration</span>
      </div>
      {#each output.shownEntries as entry (entry.id)}
        <button
          type="button"
          class="line {kindOf(entry)}"
          title="Show the output of this command"
          onclick={() => void output.openRecord(entry.id)}
        >
          <span class="when tabular">{clock.format(entry.startedAtMs)}</span>
          <span class="what truncate">{entry.operation}</span>
          <span class="where truncate" title={entry.repo}>{repoName(entry)}</span>
          <span class="cmd mono truncate">{entry.command}</span>
          <span class="num tabular">{entry.exitCode ?? "?"}</span>
          <span class="num tabular">{entry.durationMs} ms</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .output {
    display: flex;
    flex-direction: column;
    flex: 0 0 auto;
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

  .grow {
    flex: 1 1 auto;
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .line {
    display: grid;
    grid-template-columns: 72px minmax(72px, 120px) minmax(80px, 160px) minmax(0, 1fr) 40px 72px;
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

  button.line:hover {
    background: var(--state-hover);
  }

  .head {
    position: sticky;
    top: 0;
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    color: var(--text-secondary);
    font-size: var(--fs-small);
    font-weight: 600;
  }

  .when,
  .where,
  .cmd,
  .num {
    color: var(--text-secondary);
  }

  .when,
  .num {
    font-size: var(--fs-small);
  }

  .num {
    text-align: right;
  }

  .line.error .what {
    color: var(--status-delete);
  }

  .line.warning .what {
    color: var(--status-modify);
  }

  .btn.kind-error[aria-pressed="true"] {
    color: var(--status-delete);
  }

  .btn.kind-warning[aria-pressed="true"] {
    color: var(--status-modify);
  }
</style>
