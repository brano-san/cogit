<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";
  import Select from "$components/common/Select.svelte";
  import Tree from "$components/common/Tree.svelte";
  import {
    CATEGORIES,
    firstMatch,
    matchingCategories,
    disabledBy,
    restoreCategory,
    restoreKeys,
  } from "$lib/preferences";
  import type { TreeNode } from "$lib/tree";
  import { needsRestart, DEFAULT_SETTINGS, THEMES, type Settings } from "$lib/settings";
  import type { Keymap } from "$lib/keymap";
  import type { KeyBinding } from "$lib/ipc";
  import KeymapEditor from "$components/layout/KeymapEditor.svelte";
  import GraphField from "$components/layout/GraphField.svelte";
  import ToolbarEditor from "$components/layout/ToolbarEditor.svelte";
  import { DEFAULT_LAYOUT } from "$lib/toolbar";
  import { recordEdit } from "$lib/toolbar-layout";
  import { canUndo, emptyStack, undo, type UndoStack } from "$lib/undo-stack";
  import { parseChoice, suppressedChoices } from "$lib/suppressions";

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
    /** Applies and saves the whole draft, keymap included, each time a field changes (R-122). */
    onapply: (next: Settings, keymap: Keymap) => void;
    /** Puts everything back to how it was when the dialog opened, and closes. */
    onrevert: () => void;
    /** Closes and keeps: everything was applied as it was chosen. */
    onclose: () => void;
    /** Warnings ignored per repository; the exit question comes from the draft itself. */
    ignored: import("$lib/suppressions").IgnoredWarnings;
    /** Brings an ignored warning back at once, outside the draft. */
    onunignore: (root: string, warning: string) => void;
    /** The page to open on; the General page when absent. */
    start?: string;
    toolbarLayout: readonly string[];
    /** Applies a toolbar layout at once, outside the draft. */
    ontoolbar: (next: string[]) => void;
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
    onrevert,
    onclose,
    ignored,
    onunignore,
    start,
    toolbarLayout,
    ontoolbar,
  }: Props = $props();

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

  const AVATARS = [
    ["gravatar", "Show them, from Gravatar"],
    ["off", "Do not show them"],
  ] as const;

  /** Everything here is applied and saved the moment it is changed, because it is also
      shown the moment it is changed and a visible change that is not kept is a trap.
      `Cancel` is the one control that undoes; `OK`, `Esc` and the ✕ all just close
      (doc/12-risks.md, R-122). The snapshot is what `Cancel` goes back to. */
  // svelte-ignore state_referenced_locally
  let draft = $state<Settings>({ ...value });
  // svelte-ignore state_referenced_locally
  let draftKeys = $state<Keymap>({ ...keymap });
  let token = $state("");
  let search = $state("");
  // svelte-ignore state_referenced_locally
  let active = $state(start ?? "git");
  /** The toolbar edits made since this window opened, for the Toolbar page's Undo. */
  let toolbarHistory = $state.raw<UndoStack<readonly string[]>>(emptyStack());
  let collapsed = $state.raw<ReadonlySet<string>>(new Set());

  const visible = $derived(matchingCategories(search));
  const nodes = $derived(
    CATEGORIES.filter((category) => visible.includes(category.id)).map<
      TreeNode & { title: string; leaf: boolean }
    >((category) => ({
      id: category.id,
      depth: category.parent === undefined ? 0 : 1,
      children: category.parent === undefined ? true : undefined,
      title: category.title,
      leaf: category.parent !== undefined,
    })),
  );
  const current = $derived(CATEGORIES.find((category) => category.id === active));
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
    onapply(draft, draftKeys);
  }

  function changeToolbar(next: readonly string[]) {
    const edit = recordEdit(toolbarHistory, toolbarLayout, next);
    toolbarHistory = edit.history;
    ontoolbar(edit.layout);
  }

  function undoToolbar() {
    const step = undo(toolbarHistory);
    if (!step) return;
    toolbarHistory = step.stack;
    ontoolbar([...step.state]);
  }

  function onsearch(text: string) {
    search = text;
    const landing = firstMatch(text);
    if (landing) active = landing;
  }

</script>

<Dialog
  title="Preferences"
  width="min(860px, 94vw)"
  height="min(640px, 88vh)"
  flush
  {onclose}
>
  <div class="panes">
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
        <Tree
          {nodes}
          {collapsed}
          oncollapse={(next) => (collapsed = next)}
          label="Settings categories"
        >
          {#snippet row(node)}
            <button
              type="button"
              class="nav-row"
              class:heading={!node.leaf}
              class:active={active === node.id}
              disabled={!node.leaf}
              onclick={() => (active = node.id)}
            >
              <span class="truncate">{node.title}</span>
            </button>
          {/snippet}
        </Tree>
        {#if nodes.length === 0}
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
            {#if field.key === "avatars"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#if draft.avatars === "ask"}
                    <p class="ask">Not decided yet — nothing is fetched until you choose.</p>
                  {/if}
                  {#each AVATARS as [id, title] (id)}
                    <Radio name="avatars" checked={draft.avatars === id} onchange={() => set("avatars", id)} label={title} />
                  {/each}
                </div>
              </div>
            {:else if field.key === "theme"}
              <div class="row">
                <span id="f-theme">{field.label}</span>
                <Select
                  value={draft.theme}
                  options={THEMES}
                  label={field.label}
                  onchange={(next) => set("theme", next)}
                />
              </div>
            {:else if field.key === "dateFormat"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each DATE_FORMATS as [id, example] (id)}
                    <Radio name="dateFormat" checked={draft.dateFormat === id} onchange={() => set("dateFormat", id)}>
                      <span class="mono">{example}</span>
                    </Radio>
                  {/each}
                </div>
              </div>
            {:else if field.key === "pullMode"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each PULL_MODES as [id, title] (id)}
                    <Radio name="pullMode" checked={draft.pullMode === id} onchange={() => set("pullMode", id)} label={title} />
                  {/each}
                </div>
              </div>
            {:else if field.key === "backgroundFetchMinutes"}
              <label class="row">
                <span>{field.label}</span>
                <span class="slider">
                  <input
                    type="range"
                    min="0"
                    max="60"
                    step="5"
                    value={draft.backgroundFetchMinutes}
                    oninput={(e) => set("backgroundFetchMinutes", e.currentTarget.valueAsNumber)}
                  />
                  <output
                    >{draft.backgroundFetchMinutes === 0
                      ? "Off"
                      : `every ${draft.backgroundFetchMinutes} min`}</output
                  >
                </span>
              </label>
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
                    <Radio name="algorithm" checked={draft.algorithm === id} onchange={() => set("algorithm", id)} label={title} />
                  {/each}
                </div>
              </div>
            {:else if field.key === "ignoreWhitespace"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each WHITESPACE as [id, title] (id)}
                    <Radio name="ignoreWhitespace" checked={draft.ignoreWhitespace === id} onchange={() => set("ignoreWhitespace", id)} label={title} />
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
            {:else if field.key === "coloredLanes"}
              <div class="row check">
                <Checkbox checked={draft.coloredLanes} onchange={(checked) => set("coloredLanes", checked)} label={field.label} />
              </div>
            {:else if field.key === "detectMoves"}
              <div class="row check">
                <Checkbox checked={draft.detectMoves} onchange={(checked) => set("detectMoves", checked)} label={field.label} />
              </div>
            {:else if field.key === "confirmExit"}
              <div class="row check">
                <Checkbox checked={draft.confirmExit} onchange={(checked) => set("confirmExit", checked)} label={field.label} />
              </div>
            {:else if field.key === "suppressions"}
              {@const choices = suppressedChoices(draft.confirmExit, ignored)}
              <p class="row">{field.label}</p>
              {#if choices.length === 0}
                <p class="hint">Nothing is hidden: every dialog and warning still shows.</p>
              {:else}
                <ul class="suppressed">
                  {#each choices as choice (choice.id)}
                    {@const parsed = parseChoice(choice.id)}
                    <li>
                      <span class="grow">
                        {choice.label}
                        {#if choice.scope}<span class="scope">— {choice.scope}</span>{/if}
                      </span>
                      <button
                        type="button"
                        class="btn"
                        onclick={() => {
                          if (parsed.kind === "confirmExit") set("confirmExit", true);
                          else if (parsed.kind === "health") onunignore(parsed.root, parsed.warning);
                        }}>Show again</button
                      >
                    </li>
                  {/each}
                </ul>
              {/if}
            {:else if field.key === "autoUpdate"}
              <div class="row check">
                <Checkbox checked={draft.autoUpdate} onchange={(checked) => set("autoUpdate", checked)} label={field.label} />
              </div>
            {:else if field.key === "wordDiff"}
              <div class="row check nested">
                <Checkbox checked={draft.wordDiff} disabled={disabledBy(draft, field.dependsOn)} onchange={(checked) => set("wordDiff", checked)} label={field.label} />
              </div>
            {:else if field.key === "terminal"}
              <div class="row">
                <span>{field.label}</span>
                <Select
                  value={draft.terminal}
                  options={terminals.map((choice) => [choice.id, choice.label] as const)}
                  label={field.label}
                  onchange={(next) => set("terminal", next)}
                />
              </div>
            {:else if field.key === "logLevel"}
              <div class="row">
                <span>{field.label}</span>
                <Select
                  value={draft.logLevel}
                  options={LOG_LEVELS.map((level) => [level, level] as const)}
                  label={field.label}
                  onchange={(next) => set("logLevel", next)}
                />
              </div>
            {:else if field.key.startsWith("graph")}
              <GraphField {field} value={draft} onset={set} />
            {:else if field.key === "toolbar"}
              <ToolbarEditor
                layout={toolbarLayout}
                onchange={changeToolbar}
                canUndo={canUndo(toolbarHistory)}
                onundo={undoToolbar}
              />
            {:else if field.key === "keymap"}
              <KeymapEditor
                {bindings}
                overrides={draftKeys}
                onchange={(next) => {
                  // Applied as it is chosen, like every other field here (R-122).
                  draftKeys = next;
                  onapply(draft, draftKeys);
                }}
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
              <button class="btn" type="button" onclick={onforgettoken}>Forget</button>
            </div>
          {:else}
            <label class="row">
              <span>{tokenHost}</span>
              <input type="password" class="text" bind:value={token} placeholder="Access token" />
              <button class="btn" type="button" disabled={token === ""} onclick={() => onstoretoken(token)}>
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

  {#snippet footer()}
    <span class="note">{note}</span>
    <button class="btn"
      type="button"
      onclick={() => {
        if (active === "toolbar") {
          changeToolbar(DEFAULT_LAYOUT);
          return;
        }
        draft = restoreCategory(draft, active);
        draftKeys = restoreKeys(draftKeys, active);
        onapply(draft, draftKeys);
      }}
    >
      Restore Defaults
    </button>
    <button class="btn" type="button" title="Put everything back to how it was when this opened" onclick={onrevert}>
      Cancel
    </button>
    <button type="button" class="btn primary" onclick={onclose}>OK</button>
  {/snippet}
</Dialog>

<style>
  .suppressed {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .suppressed li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .suppressed .scope {
    color: var(--text-secondary);
  }

  .panes {
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
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    align-items: center;
    justify-items: start;
    gap: var(--sp-4);
    min-height: var(--h-row);
    margin-bottom: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .row.choice {
    align-items: start;
  }

  /* Controls fill their column; only the labels stay at its left edge. */
  .row > :global(:not(span)) {
    justify-self: stretch;
    width: 100%;
  }

  .row.check {
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--sp-3);
    justify-items: start;
  }

  .row.check > span {
    justify-self: start;
    text-align: left;
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

  .ask {
    margin: 0 0 var(--sp-2);
    color: var(--status-modify);
    font-size: var(--fs-header);
  }

  .empty {
    margin: 0;
    padding: var(--sp-4);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .note {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  /* A nested option that its parent has switched off says so rather than looking live. */
  label:has(input:disabled),
  .row:has(:global(input:disabled)) {
    color: var(--text-secondary);
    opacity: 0.6;
  }
</style>
