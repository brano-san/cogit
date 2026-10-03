<script lang="ts">
  import { untrack } from "svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";
  import Select from "$components/common/Select.svelte";
  import Tree from "$components/common/Tree.svelte";
  import {
    CATEGORIES,
    firstMatch,
    firstPage,
    matchRanges,
    matchingCategories,
    disabledBy,
    restoreCategory,
    restoreKeys,
  } from "$lib/preferences";
  import { NO_FILTER_FOLDS, shownFolds, toggle, toggleFilterFold, type TreeNode } from "$lib/tree";
  import { needsRestart, THEMES, type Settings } from "$lib/settings";
  import { isSetting, type Field } from "$lib/preferences";
  import { createGitChecker, describeCheck, type GitCheck } from "$lib/git-check";
  import { ON_WINDOWS } from "$lib/platform";
  import { probeGit } from "$lib/ipc";
  import type { Keymap } from "$lib/keymap";
  import type { KeyBinding } from "$lib/ipc";
  import KeymapEditor from "$components/layout/KeymapEditor.svelte";
  import GraphField from "$components/layout/GraphField.svelte";
  import ToolbarEditor from "$components/layout/ToolbarEditor.svelte";
  import { DEFAULT_LAYOUT } from "$lib/toolbar";
  import { LANE_WIDTH } from "$lib/graph-geometry";
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
    ["smart", "Yesterday · Tuesday · 09-09-26"],
    ["relative", "3 days ago"],
    ["both", "Yesterday · 1 day ago"],
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

  const DIFF_LAYOUTS = [
    ["aligned", "Aligned (1:1, filler where a side has no lines)"],
    ["compact", "Compact (each side its own lines)"],
  ] as const;

  const PULL_MODES = [
    ["ffOnly", "Refuse and let me decide"],
    ["merge", "Merge the remote branch in"],
  ] as const;

  const LOG_LEVELS = ["error", "warn", "info", "debug", "trace"] as const;
  const WEB_MENUS = [
    ["auto", "Automatic"],
    ["on", "Always"],
    ["off", "Never"],
  ] as const;

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
  /** A search opens every heading with a match; folds made while it is typed are kept
      apart, and clearing it brings back `collapsed` untouched. */
  let filterFolds = $state.raw(NO_FILTER_FOLDS);
  const shown = $derived(shownFolds(collapsed, search, filterFolds));

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
  const note = $derived(current?.note ?? "");

  let gitCheck = $state.raw<GitCheck>({ state: "idle" });
  const gitChecker = createGitChecker(probeGit, (next) => (gitCheck = next));
  $effect(() => {
    gitChecker.check(untrack(() => draft.gitPath));
    return () => gitChecker.dispose();
  });

  async function browseGit() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    // An extension filter cannot match the extensionless `git` of Unix, so only Windows gets one.
    const picked = await open({
      title: "Select Git Executable",
      multiple: false,
      directory: false,
      filters: ON_WINDOWS ? [{ name: "Git (git.exe)", extensions: ["exe"] }] : undefined,
    });
    if (typeof picked !== "string") return;
    set("gitPath", picked);
    gitChecker.check(picked);
  }

  async function browseMergeTool() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: "Select Merge Tool",
      multiple: false,
      directory: false,
      filters: ON_WINDOWS ? [{ name: "Programs", extensions: ["exe", "cmd", "bat"] }] : undefined,
    });
    if (typeof picked === "string") set("mergeExternalTool", picked);
  }

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

  function toggleNode(id: string) {
    if (search.trim() === "") collapsed = toggle(collapsed, id);
    else filterFolds = toggleFilterFold(filterFolds, search, id);
  }

  /** A page opens; a heading opens or closes, and opening it also opens its first page,
      so the content always belongs to a row the tree shows as selected. */
  function activate(id: string) {
    if (nodes.find((node) => node.id === id)?.leaf) {
      active = id;
      return;
    }
    const opening = shown.has(id);
    toggleNode(id);
    const page = opening ? firstPage(id, visible) : null;
    if (page) active = page;
  }

  /** Marks what the search found in the tree and on the page, without touching the markup:
      the CSS Custom Highlight API paints ranges of the text nodes already there. */
  let panes = $state<HTMLElement>();
  $effect(() => {
    const query = search;
    void [active, nodes];
    if (!panes || typeof CSS === "undefined" || !("highlights" in CSS)) return;
    const found = new Highlight();
    const walker = document.createTreeWalker(panes, NodeFilter.SHOW_TEXT);
    for (let text = walker.nextNode(); text; text = walker.nextNode()) {
      for (const [start, end] of matchRanges(text.textContent ?? "", query)) {
        const range = new Range();
        range.setStart(text, start);
        range.setEnd(text, end);
        found.add(range);
      }
    }
    CSS.highlights.set("settings-match", found);
    return () => CSS.highlights.delete("settings-match");
  });

</script>

<Dialog
  title="Preferences"
  width="min(860px, 94vw)"
  height="min(640px, 88vh)"
  flush
  {onclose}
>
  <div class="panes" bind:this={panes}>
    <nav aria-label="Settings categories">
      <div class="search">
        <input
          type="search"
          value={search}
          oninput={(e) => onsearch(e.currentTarget.value)}
          placeholder="Search settings"
          aria-label="Search settings"
        />
      </div>
      <div class="tree">
        <Tree
          {nodes}
          collapsed={shown}
          ontoggle={toggleNode}
          onactivate={activate}
          selected={active}
          label="Settings categories"
        >
          {#snippet row(node)}
            <!-- `holds`: the heading of the page shown, so a closed one still says where it is. -->
            <span
              class="title truncate"
              class:heading={!node.leaf}
              class:holds={current?.parent === node.id}>{node.title}</span
            >
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
              <div class="row">
                {@render caption(field)}
                <div class="path">
                  <input
                    type="text"
                    class="text"
                    aria-label={field.label}
                    value={draft.gitPath}
                    oninput={(e) => {
                      set("gitPath", e.currentTarget.value);
                      gitChecker.check(e.currentTarget.value);
                    }}
                  />
                  <button type="button" class="btn browse" title="Browse…" aria-label="Browse for the Git executable" onclick={browseGit}>...</button>
                </div>
                {#if gitCheck.state === "ok"}
                  <span class="verdict ok"><span aria-hidden="true">✓</span> {describeCheck(gitCheck)}</span>
                {:else if gitCheck.state === "bad"}
                  <button type="button" class="verdict bad" data-tip={gitCheck.reason}><span aria-hidden="true">✗</span> Not a working Git</button>
                {:else if gitCheck.state === "checking"}
                  <span class="verdict">{describeCheck(gitCheck)}</span>
                {/if}
              </div>
            {:else if field.key === "mergeExternalTool"}
              <div class="row">
                {@render caption(field)}
                <div class="path">
                  <input
                    type="text"
                    class="text"
                    aria-label={field.label}
                    value={draft.mergeExternalTool}
                    oninput={(e) => set("mergeExternalTool", e.currentTarget.value)}
                  />
                  <button type="button" class="btn browse" title="Browse…" aria-label="Browse for the merge tool" onclick={browseMergeTool}>...</button>
                </div>
              </div>
            {:else if field.key === "mergeExternalToolArgs"}
              <div class="row">
                {@render caption(field)}
                <input
                  type="text"
                  class="text"
                  aria-label={field.label}
                  placeholder={'"{base}" "{ours}" "{theirs}" "{result}"'}
                  value={draft.mergeExternalToolArgs}
                  oninput={(e) => set("mergeExternalToolArgs", e.currentTarget.value)}
                />
              </div>
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
            {:else if field.key === "diffLayout"}
              <div class="row choice" role="radiogroup" aria-label={field.label}>
                <span>{field.label}</span>
                <div class="options">
                  {#each DIFF_LAYOUTS as [id, title] (id)}
                    <Radio name="diffLayout" checked={draft.diffLayout === id} onchange={() => set("diffLayout", id)} label={title} />
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
            {:else if field.key === "diffSplit"}
              <label class="row">
                <span>{field.label}</span>
                <span class="slider">
                  <input
                    type="range"
                    min="20"
                    max="80"
                    value={Math.round(draft.diffSplit * 100)}
                    oninput={(e) => set("diffSplit", e.currentTarget.valueAsNumber / 100)}
                  />
                  <output>{Math.round(draft.diffSplit * 100)} %</output>
                </span>
              </label>
            {:else if field.key === "laneWidth"}
              <label class="row">
                <span>{field.label}</span>
                <span class="slider">
                  <input
                    type="range"
                    min={LANE_WIDTH.min}
                    max={LANE_WIDTH.max}
                    value={draft.laneWidth}
                    oninput={(e) => set("laneWidth", e.currentTarget.valueAsNumber)}
                  />
                  <output>{draft.laneWidth}px</output>
                </span>
              </label>
            {:else if field.key === "detectMoves"}
              <div class="row check">
                <Checkbox checked={draft.detectMoves} onchange={(checked) => set("detectMoves", checked)} label={field.label} />
              </div>
            {:else if field.key === "startupShowWelcome"}
              <div class="row check">
                <Checkbox
                  checked={draft.startupShowWelcome}
                  onchange={(checked) => set("startupShowWelcome", checked)}
                  label={field.label}
                />
              </div>
            {:else if field.key === "confirmExit"}
              <div class="row check">
                <Checkbox checked={draft.confirmExit} onchange={(checked) => set("confirmExit", checked)} label={field.label} />
              </div>
            {:else if field.key === "confirmLocalCheckout"}
              <div class="row check">
                <Checkbox
                  checked={draft.confirmLocalCheckout}
                  onchange={(checked) => set("confirmLocalCheckout", checked)}
                  label={field.label}
                />
              </div>
            {:else if field.key === "refsShowPseudoRefs"}
              <div class="row check">
                <Checkbox
                  checked={draft.refsShowPseudoRefs}
                  onchange={(checked) => set("refsShowPseudoRefs", checked)}
                  label={field.label}
                />
              </div>
            {:else if field.key === "suppressions"}
              {@const choices = suppressedChoices(draft.confirmExit, ignored, draft.confirmLocalCheckout)}
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
                          else if (parsed.kind === "confirmLocalCheckout") set("confirmLocalCheckout", true);
                          else if (parsed.kind === "health") onunignore(parsed.root, parsed.warning);
                        }}>Show again</button
                      >
                    </li>
                  {/each}
                </ul>
              {/if}
            {:else if field.key === "notificationsTaskbar"}
              <div class="row check">
                <Checkbox checked={draft.notificationsTaskbar} onchange={(checked) => set("notificationsTaskbar", checked)} label={field.label} />
              </div>
            {:else if field.key === "notificationsTaskbarFlash"}
              <div class="row check nested">
                <Checkbox checked={draft.notificationsTaskbarFlash} disabled={disabledBy(draft, field.dependsOn)} onchange={(checked) => set("notificationsTaskbarFlash", checked)} label={field.label} />
              </div>
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
            {:else if field.key === "uiWebMenus"}
              <div class="row">
                {@render caption(field)}
                <Select
                  value={draft.uiWebMenus}
                  options={WEB_MENUS}
                  label={field.label}
                  onchange={(next) => set("uiWebMenus", next)}
                />
              </div>
            {:else if field.key === "logLevel"}
              <div class="row">
                {@render caption(field)}
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
              A token is kept for an https:// remote only. This repository has none (it uses SSH, plain
              http or no remote), so no token is needed and none is ever sent.
            </p>
          {:else if tokenStored}
            <p class="hint wide">The token is sent over https:// only, and only to {tokenHost}.</p>
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

{#snippet caption(field: Field)}
  <span
    >{field.label}{#if isSetting(field.key) && needsRestart(field.key)}<button
        type="button"
        class="star"
        aria-label="Takes effect after a restart."
        data-tip="Takes effect after a restart.">*</button
      >{/if}</span
  >
{/snippet}

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
    flex: 0 0 clamp(172px, 36%, 248px);
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--divider);
  }

  /* The dialog's shared field is 100% wide and border-box (Dialog.svelte), and its rule
     outranks a scoped one here; the gutter is this wrapper's padding, since a margin on
     the field itself would push it past the sidebar into the content pane. */
  .search {
    flex: 0 0 auto;
    min-width: 0;
    padding: var(--sp-4);
  }

  .tree {
    flex: 1 1 auto;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
  }

  .title {
    min-width: 0;
    color: var(--text-primary);
  }

  .title.heading {
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .title.heading.holds {
    color: var(--text-primary);
  }

  :global(::highlight(settings-match)) {
    background-color: var(--search-hit);
  }

  .content {
    flex: 1 1 auto;
    min-width: 0;
    padding: var(--sp-5) var(--sp-6);
    overflow-x: hidden;
    overflow-y: auto;
    container-type: inline-size;
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
    min-width: 0;
    max-width: 100%;
  }

  .slider :global(input[type="range"]) {
    flex: 1 1 auto;
    min-width: 0;
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
    margin: 0 0 var(--sp-4) var(--sp-7);
    color: var(--text-secondary);
    font-size: 11px;
  }

  .hint.wide {
    margin-left: 0;
  }

  .path {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .path .text {
    flex: 1 1 0;
  }

  .browse {
    flex: 0 0 auto;
  }

  /* Second grid row of the settings row, under the input, never beside the label. */
  .row > .verdict {
    grid-column: 2;
    justify-self: start;
    width: auto;
  }

  .verdict {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .verdict.ok {
    color: var(--status-add);
  }

  .verdict.bad {
    color: var(--status-delete);
  }

  .star,
  .verdict.bad {
    padding: 0;
    background: none;
    border: 0;
    font: inherit;
    text-align: left;
  }

  .star {
    margin-left: var(--sp-2);
    color: var(--text-secondary);
    cursor: help;
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

  /* A narrow window: label above its control instead of a second column that cannot fit. */
  @container (max-width: 440px) {
    .row {
      grid-template-columns: minmax(0, 1fr);
      align-items: start;
    }

    .row > .verdict {
      grid-column: 1;
    }
  }
</style>
