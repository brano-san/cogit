<script lang="ts">
  import { CATEGORIES, firstMatch, matchingCategories, restoreCategory } from "$lib/preferences";
  import { needsRestart, DEFAULT_SETTINGS, type Settings } from "$lib/settings";
  import type { Keymap } from "$lib/keymap";
  import type { KeyBinding } from "$lib/ipc";
  import KeymapEditor from "$components/layout/KeymapEditor.svelte";

  interface Props {
    value: Settings;
    /** What this platform can actually offer; a choice nobody can run is not a choice. */
    terminals: readonly { id: string; label: string }[];
    keymap: Keymap;
    bindings: readonly KeyBinding[];
    /** The host of the current remote, or null when it is SSH or there is none. */
    tokenHost: string | null;
    tokenStored: boolean;
    onstoretoken: (token: string) => void;
    onforgettoken: () => void;
    /** Applies the whole draft at once; nothing is written before OK. */
    onapply: (next: Settings, keymap: Keymap) => void;
    onclose: () => void;
  }

  let {
    value,
    terminals,
    keymap,
    bindings,
    tokenHost,
    tokenStored,
    onstoretoken,
    onforgettoken,
    onapply,
    onclose,
  }: Props = $props();

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
    ["trailing", "Ignore trailing whitespace"],
    ["all", "Ignore all whitespace"],
  ] as const;

  const PULL_MODES = [
    ["ffOnly", "Refuse and let me decide"],
    ["merge", "Merge the remote branch in"],
  ] as const;

  const LOG_LEVELS = ["error", "warn", "info", "debug", "trace"] as const;

  /** Nothing is written until OK: Esc and Cancel throw the whole draft away. The snapshot
      is deliberate — the dialog is mounted fresh each time it opens. */
  // svelte-ignore state_referenced_locally
  let draft = $state<Settings>({ ...value });
  // svelte-ignore state_referenced_locally
  let draftKeys = $state<Keymap>({ ...keymap });
  let token = $state("");
  let search = $state("");
  let active = $state("git");
  let collapsed = $state.raw<ReadonlySet<string>>(new Set());

  const visible = $derived(matchingCategories(search));
  const rows = $derived(
    CATEGORIES.filter(
      (category) =>
        visible.includes(category.id) &&
        (category.parent === undefined || !collapsed.has(category.parent)),
    ),
  );
  const current = $derived(CATEGORIES.find((category) => category.id === active));
  const dirty = $derived(
    JSON.stringify(draft) !== JSON.stringify(value) ||
      JSON.stringify(draftKeys) !== JSON.stringify(keymap),
  );
  const restarts = $derived(
    (Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]).some(
      (key) => draft[key] !== value[key] && needsRestart(key),
    ),
  );
  const note = $derived(
    restarts ? "*) Some changes take effect after a restart." : (current?.note ?? ""),
  );

  function set<K extends keyof Settings>(key: K, next: Settings[K]) {
    draft = { ...draft, [key]: next };
  }

  function fold(id: string) {
    const next = new Set(collapsed);
    if (!next.delete(id)) next.add(id);
    collapsed = next;
  }

  function onsearch(text: string) {
    search = text;
    const landing = firstMatch(text);
    if (landing) active = landing;
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

<div class="dialog" role="dialog" aria-label="Preferences">
  <header>
    <h2>Preferences</h2>
    <button type="button" class="icon" onclick={onclose} aria-label="Close">✕</button>
  </header>

  <div class="body">
    <nav aria-label="Settings categories">
      <input
        type="search"
        class="search"
        value={search}
        oninput={(e) => onsearch(e.currentTarget.value)}
        placeholder="Search settings"
        aria-label="Search settings"
      />
      <div class="tree">
        {#each rows as category (category.id)}
          <button
            type="button"
            class="nav-row"
            class:heading={category.parent === undefined}
            class:active={active === category.id}
            style:padding-left={category.parent === undefined ? "var(--sp-4)" : "var(--sp-7)"}
            onclick={() =>
              category.parent === undefined ? fold(category.id) : (active = category.id)}
          >
            {#if category.parent === undefined}
              <span class="caret" aria-hidden="true">{collapsed.has(category.id) ? "▸" : "▾"}</span>
            {/if}
            <span class="truncate">{category.title}</span>
          </button>
        {/each}
        {#if rows.length === 0}
          <p class="empty">No setting matches that.</p>
        {/if}
      </div>
    </nav>

    <section class="content">
      <h3>{current?.title ?? "Preferences"}</h3>

      {#if current}
        {#each current.groups as group (group.title)}
          <div class="group">
            <span class="group-title">{group.title}</span>
            <span class="rule" aria-hidden="true"></span>
          </div>

          {#each group.fields as field (field.key)}
            {#if field.key === "theme"}
              <label class="row">
                <span>{field.label}</span>
                <select
                  value={draft.theme}
                  onchange={(e) => set("theme", e.currentTarget.value as never)}
                >
                  {#each THEMES as [id, title] (id)}<option value={id}>{title}</option>{/each}
                </select>
              </label>
            {:else if field.key === "dateFormat"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each DATE_FORMATS as [id, example] (id)}
                    <label>
                      <input
                        type="radio"
                        checked={draft.dateFormat === id}
                        onchange={() => set("dateFormat", id)}
                      />
                      <span class="mono">{example}</span>
                    </label>
                  {/each}
                </div>
              </div>
            {:else if field.key === "pullMode"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each PULL_MODES as [id, title] (id)}
                    <label>
                      <input
                        type="radio"
                        checked={draft.pullMode === id}
                        onchange={() => set("pullMode", id)}
                      />
                      <span>{title}</span>
                    </label>
                  {/each}
                </div>
              </div>
            {:else if field.key === "gitPath"}
              <label class="row">
                <span>{field.label}</span>
                <input
                  type="text"
                  class="text"
                  value={draft.gitPath}
                  oninput={(e) => set("gitPath", e.currentTarget.value)}
                />
              </label>
            {:else if field.key === "algorithm"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each ALGORITHMS as [id, title] (id)}
                    <label>
                      <input
                        type="radio"
                        checked={draft.algorithm === id}
                        onchange={() => set("algorithm", id)}
                      />
                      <span>{title}</span>
                    </label>
                  {/each}
                </div>
              </div>
            {:else if field.key === "ignoreWhitespace"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each WHITESPACE as [id, title] (id)}
                    <label>
                      <input
                        type="radio"
                        checked={draft.ignoreWhitespace === id}
                        onchange={() => set("ignoreWhitespace", id)}
                      />
                      <span>{title}</span>
                    </label>
                  {/each}
                </div>
              </div>
            {:else if field.key === "contextLines"}
              <label class="row">
                <span>{field.label}</span>
                <span class="slider">
                  <input
                    type="range"
                    min="0"
                    max="25"
                    value={draft.contextLines}
                    oninput={(e) => set("contextLines", e.currentTarget.valueAsNumber)}
                  />
                  <output>{draft.contextLines}</output>
                </span>
              </label>
            {:else if field.key === "laneWidth"}
              <label class="row">
                <span>{field.label}</span>
                <span class="slider">
                  <input
                    type="range"
                    min="8"
                    max="40"
                    value={draft.laneWidth}
                    oninput={(e) => set("laneWidth", e.currentTarget.valueAsNumber)}
                  />
                  <output>{draft.laneWidth}px</output>
                </span>
              </label>
            {:else if field.key === "detectMoves"}
              <label class="row check">
                <input
                  type="checkbox"
                  checked={draft.detectMoves}
                  onchange={(e) => set("detectMoves", e.currentTarget.checked)}
                />
                <span>{field.label}</span>
              </label>
            {:else if field.key === "wordDiff"}
              <label class="row check nested">
                <input
                  type="checkbox"
                  checked={draft.wordDiff}
                  onchange={(e) => set("wordDiff", e.currentTarget.checked)}
                />
                <span>{field.label}</span>
              </label>
            {:else if field.key === "terminal"}
              <label class="row">
                <span>{field.label}</span>
                <select
                  value={draft.terminal}
                  onchange={(e) => set("terminal", e.currentTarget.value)}
                >
                  {#each terminals as choice (choice.id)}
                    <option value={choice.id}>{choice.label}</option>
                  {/each}
                </select>
              </label>
            {:else if field.key === "logLevel"}
              <label class="row">
                <span>{field.label}</span>
                <select
                  value={draft.logLevel}
                  onchange={(e) => set("logLevel", e.currentTarget.value as never)}
                >
                  {#each LOG_LEVELS as level (level)}<option value={level}>{level}</option>{/each}
                </select>
              </label>
            {:else if field.key === "keymap"}
              <KeymapEditor
                {bindings}
                overrides={draftKeys}
                onchange={(next) => (draftKeys = next)}
              />
            {/if}

            {#if field.hint}<p class="hint">{field.hint}</p>{/if}
          {/each}
        {/each}

        {#if current.id === "auth"}
          {#if tokenHost === null}
            <p class="hint wide">
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
              <input type="password" class="text" bind:value={token} placeholder="Access token" />
              <button type="button" disabled={token === ""} onclick={() => onstoretoken(token)}>
                Store
              </button>
            </label>
          {/if}
        {/if}

        {#if current.id === "advanced"}
          <p class="hint wide">
            Nothing here yet. Cache limits and the filesystem debouncer land with the
            performance work.
          </p>
        {/if}
      {/if}
    </section>
  </div>

  <footer>
    <span class="note">{note}</span>
    <button type="button" onclick={() => (draft = restoreCategory(draft, active))}>
      Restore Defaults
    </button>
    <button type="button" onclick={onclose}>Cancel</button>
    <button type="button" class="primary" disabled={!dirty} onclick={() => onapply(draft, draftKeys)}>
      OK
    </button>
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
    width: min(860px, 94vw);
    height: min(640px, 88vh);
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
    flex: 0 0 auto;
    height: var(--h-toolbar);
    padding: 0 var(--sp-5);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .icon {
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: default;
  }

  .body {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  nav {
    display: flex;
    flex-direction: column;
    flex: 0 0 248px;
    min-height: 0;
    border-right: 1px solid var(--divider);
  }

  .search {
    flex: 0 0 auto;
    height: var(--h-input);
    margin: var(--sp-4);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .tree {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .nav-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    height: var(--h-row);
    padding-right: var(--sp-4);
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  .nav-row:hover {
    background: var(--state-hover);
  }

  .nav-row.active {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .nav-row.heading {
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .caret {
    font-size: 9px;
  }

  .content {
    flex: 1 1 auto;
    min-width: 0;
    padding: var(--sp-5) var(--sp-6);
    overflow: auto;
  }

  h3 {
    margin: 0 0 var(--sp-5);
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .group {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    margin: var(--sp-6) 0 var(--sp-4);
  }

  .group:first-of-type {
    margin-top: 0;
  }

  .group-title {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .rule {
    flex: 1 1 auto;
    height: 1px;
    background: var(--divider);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-height: var(--h-row);
    margin-bottom: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .row > span:first-child {
    flex: 0 0 200px;
  }

  .row.choice {
    align-items: flex-start;
  }

  .row.check {
    gap: var(--sp-3);
  }

  .row.check > span:first-child {
    flex: 0 0 auto;
  }

  /* A dependent switch sits under the one it depends on, as SmartGit does. */
  .row.check.nested {
    padding-left: var(--sp-7);
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .options label {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .slider {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }

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

  .hint {
    margin: 0 0 var(--sp-4) 200px;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .hint.wide {
    margin-left: 0;
  }

  .stored {
    color: var(--status-add);
  }

  .empty {
    margin: 0;
    padding: var(--sp-4);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: 0 0 auto;
    padding: var(--sp-4) var(--sp-5);
    background: var(--titlebar-bg);
    border-top: 1px solid var(--titlebar-border);
  }

  .note {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .primary {
    border-color: var(--status-ref);
  }
</style>
