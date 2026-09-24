<script lang="ts">
  import { describeCandidate } from "$lib/investigate/origin";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";

  /** DeepGit's Origin Candidates view: every plausible source, the best one chosen. */
  interface Props {
    session: InvestigateSession;
  }

  let { session }: Props = $props();

  const blockPath = $derived(session.query?.previous?.path ?? session.query?.path ?? "");

  function onkeydown(event: KeyboardEvent) {
    const count = session.report?.candidates.length ?? 0;
    if (event.key === "ArrowDown" && session.chosen < count - 1) session.choose(session.chosen + 1);
    else if (event.key === "ArrowUp" && session.chosen > 0) session.choose(session.chosen - 1);
    else if (event.key === "Enter") void session.goDeeper();
    else return;
    event.preventDefault();
  }
</script>

<section class="panel">
  <header>
    <span class="title">Origin Candidates</span>
    {#if session.report}<span class="count tabular">{session.report.candidates.length}</span>{/if}
  </header>
  {#if session.search === "searching"}
    <p class="note">Searching…</p>
  {:else if session.search === "failed"}
    <p class="note error">{session.searchError}</p>
  {:else if !session.report}
    <p class="note">Pick a line in Blame to search for where it came from.</p>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <ul class="key-list" role="listbox" aria-label="Origin candidates" tabindex="0" {onkeydown}>
      {#each session.report.candidates as candidate, index (index)}
        {@const words = describeCandidate(candidate, blockPath)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          role="option"
          aria-selected={index === session.chosen}
          class:chosen={index === session.chosen}
          onclick={() => session.choose(index)}
          ondblclick={() => void session.goDeeper(index)}
          title="Double-click to go deeper"
        >
          <div class="line">
            <span class="kind">{words.title}</span>
            {#if index === session.report.best}<span class="best">best</span>{/if}
            <span class="grow"></span>
            <span class="score tabular">{candidate.score}%</span>
          </div>
          <div class="line muted">
            <span class="where mono truncate"
              >{candidate.path}:{candidate.from}{candidate.to > candidate.from ? `–${candidate.to}` : ""}</span
            >
            <span class="likelihood {candidate.likelihood}">{candidate.likelihood}</span>
          </div>
          <div class="bar"><span style:width="{candidate.score}%"></span></div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex: 0 0 300px;
    flex-direction: column;
    min-height: 0;
    background: var(--surface-base);
    border-left: 1px solid var(--divider);
  }

  header {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-header);
  }

  .title {
    font-weight: 600;
  }

  .count {
    color: var(--text-secondary);
  }

  .note {
    margin: 0;
    padding: var(--sp-6);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .error {
    color: var(--status-delete);
  }

  ul {
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  li {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    padding: var(--sp-3) var(--sp-5);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
    cursor: default;
  }

  li:hover {
    background: var(--state-hover);
  }

  li.chosen {
    background: var(--state-selected);
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }

  .kind {
    font-weight: 600;
  }

  .best {
    padding: 0 var(--sp-2);
    color: var(--text-on-accent);
    background: var(--status-add);
    border-radius: var(--r-sm);
    font-size: 10px;
    text-transform: uppercase;
  }

  .grow {
    flex: 1 1 auto;
  }

  .where {
    flex: 1 1 auto;
    min-width: 0;
  }

  .likelihood.high {
    color: var(--status-add);
  }

  .likelihood.medium {
    color: var(--status-modify);
  }

  .likelihood.low {
    color: var(--status-delete);
  }

  .bar {
    height: 3px;
    background: var(--divider);
    border-radius: 2px;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--status-ref);
    border-radius: 2px;
  }
</style>
