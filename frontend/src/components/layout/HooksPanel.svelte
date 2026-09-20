<script lang="ts">
  import type { Hook, HookOverview, HookRun } from "$lib/ipc";

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
    onclose: () => void;
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
    onclose,
  }: Props = $props();

  const present = $derived(overview?.hooks.filter((hook) => hook.state !== "missing") ?? []);
  const absent = $derived(overview?.hooks.filter((hook) => hook.state === "missing") ?? []);

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      if (editing !== null) oncancel();
      else onclose();
    }
  }

  function label(hook: Hook): string {
    return hook.state === "disabled" ? "Enable" : "Disable";
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="dialog" role="dialog" aria-label="Hooks">
  <header>
    <h2>Hooks</h2>
    <button type="button" class="icon" onclick={onclose} aria-label="Close hooks">✕</button>
  </header>

  {#if overview}
    <p class="source">
      Git runs hooks from <code>{overview.activeDir}</code>
      {#if overview.source === "hooksPath"}
        (<code>core.hooksPath = {overview.configuredPath}</code>)
      {:else}
        (this clone only — <code>.git/hooks</code> is never cloned)
      {/if}
    </p>

    {#if overview.availablePath}
      <div class="offer">
        <span>
          This repository ships hooks in <code>{overview.availablePath}</code>, but nothing points
          at them. Read them before you turn them on: they run on every commit.
        </span>
        <button type="button" onclick={() => onadopt(overview.availablePath ?? "")}>
          Use {overview.availablePath}
        </button>
      </div>
    {/if}
  {/if}

  {#if editing !== null}
    <div class="editor">
      <label for="hook-body">{editing}</label>
      <textarea
        id="hook-body"
        spellcheck="false"
        value={body}
        oninput={(event) => onbody(event.currentTarget.value)}
      ></textarea>
      <div class="actions">
        <button type="button" onclick={oncancel}>Cancel</button>
        <button type="button" class="primary" onclick={onsave}>Save</button>
      </div>
    </div>
  {:else}
    <div class="list">
      {#if present.length > 0}
        <div class="group">Present</div>
        {#each present as hook (hook.name)}
          <div class="row">
            <span class="name" class:off={hook.state === "disabled"}>{hook.name}</span>
            <span class="detail truncate">{hook.description}</span>
            {#if !hook.executable}
              <span class="warn" title="Git will not run a hook without the execution bit">
                not executable
              </span>
            {/if}
            <button
              type="button"
              disabled={running || hook.state !== "enabled"}
              onclick={() => onrun(hook.name)}>Run</button
            >
            <button type="button" onclick={() => onedit(hook.name)}>Edit</button>
            <button type="button" onclick={() => ontoggle(hook.name, hook.state === "disabled")}>
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
          <button type="button" onclick={() => onedit(hook.name)}>Create</button>
        </div>
      {/each}
    </div>
  {/if}

  {#if lastRun}
    <div class="result" class:failed={lastRun.exitCode !== 0}>
      <div class="summary">
        <strong>{lastRun.name}</strong>
        exited {lastRun.exitCode ?? "on a signal"} in {lastRun.durationMs} ms
        {#if lastRun.slow}<span class="warn">— slow enough to be felt on every commit</span>{/if}
      </div>
      {#if lastRun.stdout}<pre>{lastRun.stdout}</pre>{/if}
      {#if lastRun.stderr}<pre class="err">{lastRun.stderr}</pre>{/if}
    </div>
  {/if}
</div>

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

  button:disabled {
    opacity: 0.45;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: rgb(0 0 0 / 35%);
  }

  .dialog {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    width: min(760px, 92vw);
    max-height: 84vh;
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--h-toolbar);
    padding: 0 var(--sp-5);
    border-bottom: 1px solid var(--divider);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .source {
    margin: 0;
    padding: var(--sp-4) var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--divider);
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

  .offer button {
    flex: 0 0 auto;
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .group {
    padding: var(--sp-3) var(--sp-5) var(--sp-2, 3px);
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
    padding: var(--sp-5);
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

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-4);
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--fs-header);
  }

  button {
    height: var(--h-input);
    padding: 0 var(--sp-4);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button:hover {
    background: var(--state-hover);
  }

  button.primary {
    background: var(--status-ref);
    color: var(--c-text-inverse);
    border-color: transparent;
  }

  button.icon {
    height: 22px;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
  }
</style>
