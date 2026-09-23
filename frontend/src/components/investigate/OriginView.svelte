<script lang="ts">
  import { fileName } from "$lib/investigate/params";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";

  /** DeepGit's Origin view: the chosen candidate beside the block, with what differs
      between the two marked. */
  interface Props {
    session: InvestigateSession;
  }

  let { session }: Props = $props();

  const candidate = $derived(session.candidate);
  const blockLines = $derived.by(() => {
    const block = session.block;
    const lines = session.blame?.lines ?? [];
    return block ? lines.slice(block.start, block.end) : [];
  });
  const introducedIn = $derived(session.query?.commit.slice(0, 7) ?? "");
</script>

<section class="panel">
  {#if !candidate}
    <p class="note">Choose an origin candidate to compare it with the selected lines.</p>
  {:else}
    <div class="side">
      <header class="truncate">
        Selected lines · <span class="mono">{fileName(session.location.path)}</span>
      </header>
      <div class="code">
        {#each blockLines as line, index (index)}
          <div class="row {candidate.block[index] ?? 'missing'}">
            <span class="number tabular">{line.line}</span><span class="mono text">{line.text}</span>
          </div>
        {/each}
      </div>
    </div>
    <div class="side">
      <header class="truncate">
        {#if candidate.kind === "appeared"}
          Nothing earlier: the lines appeared in <span class="mono">{introducedIn}</span>
        {:else}
          <span class="mono">{candidate.path}</span> before <span class="mono">{introducedIn}</span>
        {/if}
      </header>
      <div class="code">
        {#each candidate.source as line (line.line)}
          <div class="row {line.status}">
            <span class="number tabular">{line.line}</span><span class="mono text">{line.text}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    background: var(--surface-base);
    border-left: 1px solid var(--divider);
  }

  .note {
    margin: 0;
    padding: var(--sp-6);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .side {
    display: flex;
    flex: 1 1 50%;
    flex-direction: column;
    min-width: 0;
  }

  .side + .side {
    border-left: 1px solid var(--divider);
  }

  header {
    flex: none;
    height: var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    line-height: var(--h-panel-hdr);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-header);
  }

  .code {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .row {
    display: flex;
    gap: var(--sp-4);
    height: 18px;
    padding: 0 var(--sp-4);
    white-space: pre;
  }

  /* DeepGit marks the differences between the two blocks in dark yellow. */
  .row.changed {
    background: var(--c-modified-bg);
  }

  .row.missing {
    opacity: 0.55;
  }

  .number {
    flex: 0 0 5ch;
    color: var(--text-secondary);
    text-align: right;
    user-select: none;
  }

  .text {
    line-height: 18px;
  }
</style>
