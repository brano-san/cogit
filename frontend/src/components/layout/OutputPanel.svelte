<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import type { GitOutput } from "$lib/ipc";
  import { highlightStream } from "$lib/output-highlight";
  import { isFailure, isWarning, output } from "$stores/output.svelte";

  let expanded = $state<Set<string>>(new Set());

  function key(entry: GitOutput, index: number): string {
    return `${index}:${entry.command}`;
  }

  function toggle(id: string) {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expanded = next;
  }

  function asText(entry: GitOutput): string {
    return [
      `$ ${entry.command}`,
      `exit ${entry.exitCode ?? "?"} in ${entry.durationMs} ms`,
      entry.stdout,
      entry.stderr,
    ]
      .filter((part) => part.trim() !== "")
      .join("\n");
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && output.open) output.open = false;
  }
</script>

<svelte:window {onkeydown} />

<div class="output">
  <header>
    <span class="title">Output</span>
    <label><input type="checkbox" bind:checked={output.errorsOnly} /> Problems only</label>
    <span class="grow"></span>
    <button type="button" onclick={() => void writeText(output.shown.map(asText).join("\n\n"))}>
      Copy log
    </button>
    <button type="button" onclick={() => void output.clear()}>Clear</button>
    <button type="button" onclick={() => (output.open = false)} title="Close (Esc)">✕</button>
  </header>

  {#if output.shown.length === 0}
    <p class="message">No Git command has run yet.</p>
  {:else}
    <div class="list">
      {#each output.shown as entry, index (key(entry, index))}
        {@const id = key(entry, index)}
        <div class="entry" class:failed={isFailure(entry)} class:warned={isWarning(entry)}>
          <button type="button" class="line" onclick={() => toggle(id)}>
            <span class="caret">{expanded.has(id) ? "▾" : "▸"}</span>
            <span class="cmd mono truncate">{entry.command}</span>
            <span class="meta tabular">{entry.exitCode ?? "?"} · {entry.durationMs} ms</span>
          </button>
          {#if expanded.has(id)}
            {#each [entry.stdout, entry.stderr] as stream, which (which)}
              {#if stream.trim() !== ""}
                <pre class="stream mono">{#each highlightStream(stream) as line, at (at)}<span
                      class="ln {line.kind}">{line.text}</span
                    >{/each}</pre>
              {/if}
            {/each}
          {/if}
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

  label {
    display: flex;
    align-items: center;
    gap: var(--sp-2, 3px);
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }

  header button {
    height: 20px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
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

  .caret {
    flex: 0 0 auto;
    width: 10px;
    color: var(--text-secondary);
  }

  .cmd {
    flex: 1 1 auto;
    min-width: 0;
  }

  .meta {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .entry.failed .cmd {
    color: var(--status-delete);
  }

  .entry.warned .cmd {
    color: var(--status-modify);
  }

  /* Raw Git output is never reformatted or truncated (INV-05). */
  .stream {
    margin: 0 var(--sp-5) var(--sp-3) calc(var(--sp-5) + 13px);
    padding: var(--sp-3);
    background: var(--surface-input);
    border-radius: var(--r-sm);
    font-size: var(--fs-code);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  /* One block per line, so a blank line keeps its height without a literal newline. */
  .ln {
    display: block;
    min-height: 1em;
  }

  .ln.error {
    color: var(--status-delete);
  }

  .ln.warning {
    color: var(--status-modify);
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    color: var(--text-secondary);
  }
</style>
