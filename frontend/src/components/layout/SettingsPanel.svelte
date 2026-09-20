<script lang="ts">
  import { needsRestart, type Settings } from "$lib/settings";

  interface Props {
    value: Settings;
    /** The host of the current remote, or null when it is SSH or there is none. */
    tokenHost: string | null;
    tokenStored: boolean;
    onstoretoken: (token: string) => void;
    onforgettoken: () => void;
    onchange: <K extends keyof Settings>(key: K, next: Settings[K]) => void;
    onreset: () => void;
    onclose: () => void;
  }

  let {
    value,
    tokenHost,
    tokenStored,
    onstoretoken,
    onforgettoken,
    onchange,
    onreset,
    onclose,
  }: Props = $props();

  let token = $state("");

  const THEMES = [
    ["dark", "Dark"],
    ["light", "Light"],
  ] as const;

  // Labelled by example: the setting is about what the row will read, not about a term.
  const DATE_FORMATS = [
    ["smart", "yesterday · Tuesday · 09-09-26"],
    ["relative", "3 days ago"],
    ["both", "yesterday · 1 day ago"],
  ] as const;

  const ALGORITHMS = [
    ["histogram", "Histogram"],
    ["myers", "Myers"],
  ] as const;

  const WHITESPACE = [
    ["none", "Show every change"],
    ["trailing", "Ignore trailing"],
    ["all", "Ignore all whitespace"],
  ] as const;

  const PULL_MODES = [
    ["ffOnly", "Fast-forward only"],
    ["merge", "Merge"],
  ] as const;

  const LOG_LEVELS = ["error", "warn", "info", "debug", "trace"] as const;

  let dirty = $state<Set<string>>(new Set());

  function set<K extends keyof Settings>(key: K, next: Settings[K]) {
    if (needsRestart(key)) dirty = new Set([...dirty, key]);
    onchange(key, next);
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

<div class="dialog" role="dialog" aria-label="Settings">
  <header>
    <h2>Settings</h2>
    <button type="button" class="icon" onclick={onclose} aria-label="Close settings">✕</button>
  </header>

  <div class="body">
    <section>
      <h3>Appearance</h3>

      <label class="row">
        <span>Theme</span>
        <select value={value.theme} onchange={(e) => set("theme", e.currentTarget.value as never)}>
          {#each THEMES as [id, title] (id)}<option value={id}>{title}</option>{/each}
        </select>
      </label>

      <label class="row">
        <span>Dates</span>
        <select
          value={value.dateFormat}
          onchange={(e) => set("dateFormat", e.currentTarget.value as never)}
        >
          {#each DATE_FORMATS as [id, title] (id)}<option value={id}>{title}</option>{/each}
        </select>
      </label>
    </section>

    <section>
      <h3>Diff</h3>

      <label class="row">
        <span>Algorithm</span>
        <select
          value={value.algorithm}
          onchange={(e) => set("algorithm", e.currentTarget.value as never)}
        >
          {#each ALGORITHMS as [id, title] (id)}<option value={id}>{title}</option>{/each}
        </select>
      </label>

      <label class="row">
        <span>Whitespace</span>
        <select
          value={value.ignoreWhitespace}
          onchange={(e) => set("ignoreWhitespace", e.currentTarget.value as never)}
        >
          {#each WHITESPACE as [id, title] (id)}<option value={id}>{title}</option>{/each}
        </select>
      </label>

      <label class="row">
        <span>Context lines</span>
        <span class="slider">
          <input
            type="range"
            min="0"
            max="25"
            value={value.contextLines}
            oninput={(e) => set("contextLines", e.currentTarget.valueAsNumber)}
          />
          <output>{value.contextLines}</output>
        </span>
      </label>

      <label class="row check">
        <input
          type="checkbox"
          checked={value.wordDiff}
          onchange={(e) => set("wordDiff", e.currentTarget.checked)}
        />
        <span>Highlight changed words inside a line</span>
      </label>

      <label class="row check">
        <input
          type="checkbox"
          checked={value.detectMoves}
          onchange={(e) => set("detectMoves", e.currentTarget.checked)}
        />
        <span>Mark moved blocks instead of delete plus insert</span>
      </label>
    </section>

    <section>
      <h3>Graph</h3>

      <label class="row">
        <span>Lane width</span>
        <span class="slider">
          <input
            type="range"
            min="8"
            max="40"
            value={value.laneWidth}
            oninput={(e) => set("laneWidth", e.currentTarget.valueAsNumber)}
          />
          <output>{value.laneWidth}px</output>
        </span>
      </label>
    </section>

    <section>
      <h3>Credentials</h3>

      {#if tokenHost === null}
        <p class="hint">
          This repository authenticates over SSH or has no remote, so no token is needed.
        </p>
      {:else if tokenStored}
        <div class="row">
          <span>{tokenHost}</span>
          <span class="stored">A token is stored.</span>
          <button type="button" onclick={onforgettoken}>Forget</button>
        </div>
      {:else}
        <label class="row">
          <span>{tokenHost}</span>
          <input
            type="password"
            class="text"
            bind:value={token}
            placeholder="Personal access token"
            autocomplete="off"
          />
        </label>
        <div class="row">
          <span></span>
          <button
            type="button"
            disabled={token.trim() === ""}
            onclick={() => {
              onstoretoken(token.trim());
              token = "";
            }}>Store in the OS keychain</button
          >
        </div>
      {/if}
    </section>

    <section>
      <h3>Git</h3>

      <label class="row">
        <span>Pull</span>
        <select
          value={value.pullMode}
          onchange={(e) => set("pullMode", e.currentTarget.value as never)}
        >
          {#each PULL_MODES as [id, title] (id)}<option value={id}>{title}</option>{/each}
        </select>
      </label>

      <label class="row">
        <span>Git executable</span>
        <input
          type="text"
          class="text"
          value={value.gitPath}
          onchange={(e) => set("gitPath", e.currentTarget.value)}
        />
      </label>

      <label class="row">
        <span>Log level</span>
        <select
          value={value.logLevel}
          onchange={(e) => set("logLevel", e.currentTarget.value as never)}
        >
          {#each LOG_LEVELS as level (level)}<option value={level}>{level}</option>{/each}
        </select>
      </label>

      {#if dirty.size > 0}
        <p class="restart">Restart Cogit to apply: {[...dirty].join(", ")}.</p>
      {/if}
    </section>
  </div>

  <footer>
    <button type="button" onclick={onreset}>Restore Defaults</button>
    <button type="button" class="primary" onclick={onclose}>Close</button>
  </footer>
</div>

<style>
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
    width: min(520px, 88vw);
    max-height: 80vh;
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

  h3 {
    margin: 0 0 var(--sp-3);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .body {
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-5);
    overflow: auto;
  }

  section + section {
    margin-top: var(--sp-6);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    min-height: var(--h-row);
    font-size: var(--fs-dense);
  }

  .row > span:first-child {
    flex: 0 0 140px;
    color: var(--text-secondary);
  }

  .row.check > span {
    flex: 1 1 auto;
    color: var(--text-primary);
  }

  select,
  .text {
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

  .slider {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    gap: var(--sp-4);
  }

  .slider input {
    flex: 1 1 auto;
    min-width: 0;
    accent-color: var(--status-ref);
  }

  output {
    flex: 0 0 44px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--fs-header);
    text-align: right;
  }

  .hint,
  .stored {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .hint {
    margin: 0;
  }

  button:disabled {
    opacity: 0.5;
  }

  .restart {
    margin: var(--sp-4) 0 0;
    color: var(--status-modify);
    font-size: var(--fs-header);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
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

  button.icon {
    height: 22px;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
  }
</style>
