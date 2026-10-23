<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import type { Bypass, Hook, HookOverview, HookRun, PresetStatus } from "$lib/ipc";
  import { presetProblems } from "$lib/preset-tool";

  interface Props {
    overview: HookOverview | null;
    editing: string | null;
    body: string;
    onedit: (name: string) => void;
    onbody: (text: string) => void;
    onsave: () => void;
    oncancel: () => void;
    ontoggle: (name: string, enabled: boolean) => void;
    onadopt: (path: string) => void;
    onrun: (name: string) => void;
    lastRun: HookRun | null;
    running: boolean;
    bypasses: readonly Bypass[];
    presets: readonly PresetStatus[];
    showPresets: boolean;
    ontogglepresets: () => void;
    oninstall: (id: string) => void;
    onexport: (hook: string) => void;
    onremovepreset: (id: string) => void;
    onclose: () => void;
    /** The hook in the editor has changes not saved yet. */
    dirty?: boolean;
  }

  let {
    overview,
    editing,
    body,
    onedit,
    onbody,
    onsave,
    oncancel,
    ontoggle,
    onadopt,
    onrun,
    lastRun,
    running,
    bypasses,
    presets,
    showPresets,
    ontogglepresets,
    oninstall,
    onexport,
    onremovepreset,
    onclose,
    dirty = false,
  }: Props = $props();

  const present = $derived(overview?.hooks.filter((hook) => hook.state !== "missing") ?? []);
  const absent = $derived(overview?.hooks.filter((hook) => hook.state === "missing") ?? []);
  const inEditor = $derived(editing !== null && !showPresets);

  function label(hook: Hook): string {
    return hook.state === "disabled" ? "Enable" : "Disable";
  }
</script>

<!-- Esc and the ✕ leave the editor first, asking when the hook has changes; the scrim then
     leaves it open (R-515). -->
<Dialog
  title="Hooks"
  onclose={() => (inEditor ? oncancel() : onclose())}
  dirty={inEditor && dirty}
  width="min(760px, 92vw)"
  height={showPresets ? undefined : "min(720px, 84vh)"}
  flush
>
  {#if overview}
    <div class="source">
      <div>Git runs hooks from <code>{overview.activeDir}</code></div>
      <div class="note">
        {#if overview.source === "hooksPath"}
          Set by <code>core.hooksPath = {overview.configuredPath}</code>
        {:else}
          This clone only — <code>.git/hooks</code> is never cloned
        {/if}
      </div>
    </div>

    {#if overview.availablePath}
      <div class="offer">
        <span>
          This repository ships hooks in <code>{overview.availablePath}</code>, but nothing points
          at them. Read them before you turn them on: they run on every commit.
        </span>
        <button type="button" class="btn" onclick={() => onadopt(overview.availablePath ?? "")}>
          Use {overview.availablePath}
        </button>
      </div>
    {/if}
  {/if}

  {#if bypasses.length > 0}
    <p class="bypasses">
      {bypasses.length} commit{bypasses.length === 1 ? "" : "s"} in this clone skipped the hooks.
      Most recent: <code>{bypasses[0]?.oid.slice(0, 7)}</code> {bypasses[0]?.summary}
    </p>
  {/if}

  {#if showPresets}
    <!-- One grid, rows on its columns (subgrid): the header sticks while the rows scroll, and
         only Description gives way when the window narrows. -->
    <div class="list presets" role="table" aria-label="Hook presets">
      <div class="prow head" role="row">
        <span role="columnheader">Preset</span>
        <span role="columnheader">Hook</span>
        <span role="columnheader">Description</span>
        <span role="columnheader">Requirements</span>
        <span role="columnheader" aria-label="Actions"></span>
      </div>
      {#each presets as entry (entry.id)}
        {@const problems = presetProblems(entry)}
        <div class="prow" role="row">
          <span class="pname" role="cell">
            {entry.name}
            {#if entry.user}<span class="badge mine">yours</span>{/if}
          </span>
          <span role="cell"><span class="badge hook">{entry.hook}</span></span>
          <span class="clamp" role="cell" title={entry.description}>{entry.description}</span>
          <span class="reqs" role="cell">
            {#each problems as problem (problem.text)}
              <span class="badge problem" title={problem.tip}>
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M8 2 14.5 13.5h-13Z" />
                  <path d="M8 6.5v3.5M8 11.8v.2" />
                </svg>
                {problem.text}
              </span>
            {:else}
              <svg class="ok" viewBox="0 0 16 16" role="img" aria-label="Ready">
                <path d="M3.5 8.5 6.5 11.5 12.5 4.5" />
              </svg>
            {/each}
          </span>
          <span class="acts" role="cell">
            <button type="button" class="btn" onclick={() => oninstall(entry.id)}>Install</button>
            {#if entry.user}
              <button type="button" class="btn" onclick={() => onremovepreset(entry.id)}>Remove</button>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  {:else if editing !== null}
    <div class="editor">
      <label for="hook-body">{editing}</label>
      <textarea
        id="hook-body"
        spellcheck="false"
        value={body}
        oninput={(event) => onbody(event.currentTarget.value)}
      ></textarea>
    </div>
  {:else}
    <div class="list">
      {#if present.length > 0}
        <div class="group">Present</div>
        {#each present as hook (hook.name)}
          <div class="row">
            <span class="name" class:off={hook.state === "disabled"}>{hook.name}</span>
            <button type="button" class="btn" title="Save this hook as a preset"
              onclick={() => onexport(hook.name)}>Save as preset</button
            >
            <span class="detail truncate">{hook.description}</span>
            {#if !hook.executable}
              <span class="warn" title="Git will not run a hook without the execution bit">
                not executable
              </span>
            {/if}
            <button
              type="button"
              class="btn"
              disabled={running || hook.state !== "enabled"}
              onclick={() => onrun(hook.name)}>Run</button
            >
            <button type="button" class="btn" onclick={() => onedit(hook.name)}>Edit</button>
            <button type="button" class="btn" onclick={() => ontoggle(hook.name, hook.state === "disabled")}>
              {label(hook)}
            </button>
          </div>
        {/each}
      {/if}

      <div class="group">Not set up</div>
      {#each absent as hook (hook.name)}
        <div class="row">
          <span class="name muted">{hook.name}</span>
          <span class="detail truncate">{hook.description}</span>
          <button type="button" class="btn" onclick={() => onedit(hook.name)}>Create</button>
        </div>
      {/each}
    </div>
  {/if}

  {#if lastRun}
    <div class="result" class:failed={lastRun.exitCode !== null && lastRun.exitCode !== 0}>
      <div class="summary">
        <strong>{lastRun.name}</strong>
        {lastRun.exitCode === null ? "stopped" : `exited ${lastRun.exitCode}`} after {lastRun.durationMs} ms
        {#if lastRun.slow}<span class="warn">— slow enough to be felt on every commit</span>{/if}
      </div>
      {#if lastRun.stdout}<pre>{lastRun.stdout}</pre>{/if}
      {#if lastRun.stderr}<pre class="err">{lastRun.stderr}</pre>{/if}
    </div>
  {/if}

  {#snippet footer()}
    {#if inEditor}
      <button type="button" class="btn" onclick={oncancel}>Cancel</button>
      <button type="button" class="btn primary" onclick={onsave}>Save</button>
    {:else}
      <button type="button" class="btn" onclick={ontogglepresets}>
        {showPresets ? "Back to hooks" : "Presets…"}
      </button>
      <span class="grow"></span>
      <button type="button" class="btn primary" data-autofocus onclick={onclose}>Close</button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .result {
    flex: 0 0 auto;
    max-height: 30vh;
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
    overflow: auto;
  }

  .result.failed {
    background: var(--c-deleted-bg);
  }

  .summary {
    font-size: var(--fs-dense);
  }

  .result pre {
    margin: var(--sp-3) 0 0;
    color: var(--text-code);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    white-space: pre-wrap;
  }

  .result pre.err {
    color: var(--status-delete);
  }

  .source {
    flex: 0 0 auto;
    padding: var(--sp-4) var(--sp-5);
    color: var(--text-primary);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--divider);
  }

  .source .note {
    margin-top: var(--sp-1);
    color: var(--text-secondary);
  }

  .offer {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    padding: var(--sp-4) var(--sp-5);
    background: var(--c-modified-bg);
    color: var(--text-primary);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--divider);
  }

  .offer .btn {
    flex: 0 0 auto;
  }

  .bypasses {
    margin: 0;
    padding: var(--sp-4) var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--divider);
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .group {
    padding: var(--sp-3) var(--sp-5) var(--sp-2);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-height: var(--h-row);
    padding: var(--sp-2) var(--sp-5);
    font-size: var(--fs-dense);
  }

  .row:hover {
    background: var(--state-hover);
  }

  .presets {
    display: grid;
    grid-template-columns: fit-content(16em) max-content minmax(0, 1fr) max-content max-content;
    align-content: start;
    font-size: var(--fs-dense);
  }

  .prow {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    column-gap: var(--sp-5);
    /* Two lines of description plus padding: every row is as tall as the tallest can be. */
    min-height: calc(2lh + 2 * var(--sp-3));
    padding: var(--sp-3) var(--sp-5);
    border-bottom: 1px solid var(--divider);
  }

  .prow:not(.head):hover {
    background: var(--state-hover);
  }

  .prow.head {
    position: sticky;
    top: 0;
    z-index: 1;
    min-height: 0;
    padding-block: var(--sp-3);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
  }

  .pname {
    min-width: 0;
    color: var(--text-primary);
  }

  .clamp {
    display: -webkit-box;
    min-width: 0;
    overflow: hidden;
    color: var(--text-secondary);
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 0 var(--sp-3);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    font-size: var(--fs-header);
    white-space: nowrap;
  }

  .badge.hook {
    font-family: var(--font-mono);
    color: var(--text-primary);
  }

  .badge.mine {
    margin-left: var(--sp-2);
    color: var(--status-ref);
    border-color: var(--status-ref);
  }

  .reqs {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-1);
  }

  .badge.problem {
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
    border-color: transparent;
  }

  .reqs svg {
    flex: none;
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .reqs .ok {
    width: 14px;
    height: 14px;
    color: var(--text-secondary);
  }

  .acts {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-3);
  }

  .acts .btn {
    min-width: 76px;
    justify-content: center;
  }

  .name {
    flex: 0 0 170px;
    font-family: var(--font-mono);
  }

  .name.off {
    color: var(--text-secondary);
    text-decoration: line-through;
  }

  .muted {
    color: var(--text-secondary);
  }

  .detail {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
  }

  .warn {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-size: var(--fs-header);
  }

  .editor {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--sp-3);
    min-height: 0;
    padding: var(--sp-5) var(--sp-5);
  }

  .editor label {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
  }

  textarea {
    flex: 1 1 auto;
    min-height: 260px;
    padding: var(--sp-4);
    background: var(--surface-input);
    color: var(--text-code);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    line-height: var(--lh-code);
    resize: none;
    white-space: pre;
  }

  .grow {
    flex: 1 1 auto;
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--fs-header);
  }
</style>
