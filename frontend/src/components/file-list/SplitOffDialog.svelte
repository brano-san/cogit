<script lang="ts">
  import { splitProblem, splitSummary } from "$lib/split-off";

  interface Props {
    oid: string;
    changed: readonly string[];
    published: boolean;
    busy: boolean;
    onsplit: (paths: string[], message: string, splitFirst: boolean) => void;
    onclose: () => void;
  }

  let { oid, changed, published, busy, onsplit, onclose }: Props = $props();

  let chosen = $state<string[]>([]);
  let message = $state("");
  let splitFirst = $state(true);

  const problem = $derived(splitProblem(changed, chosen, message));
  const summary = $derived(splitSummary(changed, chosen, splitFirst));

  function toggle(path: string) {
    chosen = chosen.includes(path) ? chosen.filter((p) => p !== path) : [...chosen, path];
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="dialog" role="dialog" aria-label="Split off files">
  <header>
    <h2>Split off files from {oid.slice(0, 7)}</h2>
    <button type="button" class="icon" onclick={onclose} aria-label="Close">✕</button>
  </header>

  {#if published}
    <p class="danger">
      This commit is already on a remote. Splitting it rewrites every commit from here on, so
      the branch will need a force-push and anyone who pulled it will have to reset.
    </p>
  {/if}

  <div class="files">
    {#each changed as path (path)}
      <label class="row">
        <input type="checkbox" checked={chosen.includes(path)} onchange={() => toggle(path)} />
        <span class="truncate">{path}</span>
      </label>
    {/each}
  </div>

  <div class="form">
    <label class="field">
      <span>New commit message</span>
      <input type="text" bind:value={message} placeholder="What the split-off files do" />
    </label>

    <div class="field">
      <span>Position</span>
      <div class="choice">
        <label>
          <input type="radio" checked={splitFirst} onchange={() => (splitFirst = true)} />
          Before the original
        </label>
        <label>
          <input type="radio" checked={!splitFirst} onchange={() => (splitFirst = false)} />
          After the original
        </label>
      </div>
    </div>

    <p class="summary">{summary}</p>
  </div>

  <footer>
    {#if problem}<span class="problem">{problem}</span>{/if}
    <button type="button" onclick={onclose}>Cancel</button>
    <button
      type="button"
      class="primary"
      disabled={problem !== null || busy}
      onclick={() => onsplit(chosen, message.trim(), splitFirst)}
    >
      {busy ? "Splitting…" : "Split Off"}
    </button>
  </footer>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--scrim);
  }

  .dialog {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    width: min(560px, 90vw);
    max-height: 82vh;
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

  .danger {
    margin: 0;
    padding: var(--sp-4) var(--sp-5);
    background: var(--c-deleted-bg);
    color: var(--text-primary);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--divider);
  }

  .files {
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-3) 0;
    overflow: auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
  }

  .row:hover {
    background: var(--state-hover);
  }

  .form {
    flex: 0 0 auto;
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    min-height: var(--h-row);
    font-size: var(--fs-dense);
  }

  .field > span:first-child {
    flex: 0 0 150px;
    color: var(--text-secondary);
  }

  .field input[type="text"] {
    flex: 1 1 auto;
    min-width: 0;
    height: var(--h-input);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .choice {
    display: flex;
    gap: var(--sp-6);
  }

  .choice label {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .summary {
    margin: var(--sp-3) 0 0;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
  }

  .problem {
    flex: 1 1 auto;
    color: var(--status-modify);
    font-size: var(--fs-header);
  }

  button {
    height: var(--h-input);
    padding: 0 var(--sp-5);
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

  button:disabled {
    opacity: 0.45;
  }

  button.icon {
    height: 22px;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
  }
</style>
