<script lang="ts">
  import FileStateIcon from "$components/common/FileStateIcon.svelte";
  import Icon from "$components/common/Icon.svelte";
  import MenuMark from "$components/common/MenuMark.svelte";
  import { GLYPHS } from "$lib/icons";
  import { columnItems, toggleColumn } from "$lib/file-columns";
  import type { FileView } from "$lib/file-view";
  import { layoutToggle, RENAME_SOURCES_TITLE, stateSwitches, toolReason, type ListContext } from "$lib/file-switches";
  import { filesView } from "$stores/files-view.svelte";

  /**
   * The bar above the file list (issue 11). Left: what is hidden and how to search.
   * Right: how the list is split, how it is structured, and which states it shows.
   */
  interface Props {
    view: FileView;
    onview: (next: FileView) => void;
    filter: string;
    onfilter: (text: string) => void;
    /** Files of the revision the list does not show (R-871); `null` while unknown. */
    hidden: number | null;
    /** The filter text is not a valid expression; the field says so quietly. */
    broken?: boolean;
    /** No repository behind the list. */
    disabled?: boolean;
    /** Content search reads the files on disk, so only the working tree has it. */
    contentsReady?: boolean;
    /** A commit or a stash has fewer switches that mean anything (#3). */
    context?: ListContext;
  }

  let {
    view,
    onview,
    filter,
    onfilter,
    hidden,
    broken = false,
    disabled = false,
    contentsReady = false,
    context = "worktree",
  }: Props = $props();

  let box: HTMLInputElement | undefined = $state();
  let columnsOpen = $state(false);
  let menuButton: HTMLButtonElement | undefined = $state();
  /** The bar clips what overflows it, so the menu is placed on the page, not in the bar. */
  let menuAt = $state({ top: 0, right: 0 });

  function toggleMenu() {
    const box = menuButton?.getBoundingClientRect();
    if (!columnsOpen && box) menuAt = { top: box.bottom + 2, right: window.innerWidth - box.right };
    columnsOpen = !columnsOpen;
  }

  export function focus() {
    box?.select();
  }

  const set = (key: keyof FileView, value: boolean) => onview({ ...view, [key]: value });

  const LAYOUT_GLYPH = { tree: "directories", flat: "flatList" } as const;

  /** Shown only beside Show directories turned on; Expand All while anything is folded. */
  const foldAll = $derived(view.directories && !disabled);
  const foldAllTitle = $derived(filesView.anyCollapsed ? "Expand All Folders" : "Collapse All Folders");

  const switches = $derived(stateSwitches(context));
  const layout = $derived(layoutToggle(view.directories));
  const splitReason = $derived(toolReason(context, "separateIndex"));
  const contentsReason = $derived(
    toolReason(context, "contents") ??
      (contentsReady ? null : "Search in file contents — only the working tree is on disk to search"),
  );

  /** Customize View (#34): the columns of the table. On a
      commit the sources of renames are the Missing switch; the working tree has them here. */
  const columnMenu = $derived(columnItems(filesView.columns, view.directories));
</script>

<div class="bar" role="toolbar" aria-label="File list options">
  {#if hidden !== null}
    {@const said = `${hidden} file${hidden === 1 ? "" : "s"} hidden`}
    <span class="note" title="{said}: files of this revision the list does not show">
      <span class="count tabular">{hidden}</span><span class="words truncate">&nbsp;file{hidden === 1 ? "" : "s"} hidden</span>
    </span>
  {/if}

  <div class="field" class:broken>
    <input
      bind:this={box}
      type="search"
      value={filter}
      placeholder="File Filter"
      aria-label="Filter files by name or path"
      title="Filter files by name or path (Ctrl+F)"
      {disabled}
      oninput={(event) => onfilter(event.currentTarget.value)}
    />
    <button
      type="button"
      class="chip"
      aria-pressed={view.regex}
      title="Use Regular Expressions"
      {disabled}
      onclick={() => set("regex", !view.regex)}>*</button
    >
    <button
      type="button"
      class="chip"
      class:dead={contentsReason !== null}
      aria-pressed={view.contents && contentsReason === null}
      aria-disabled={contentsReason !== null}
      title={contentsReason ?? "Search in file contents"}
      {disabled}
      onclick={() => contentsReason === null && set("contents", !view.contents)}>⌕</button
    >
  </div>

  <span class="spacer"></span>

  <button
    type="button"
    class="tool"
    class:dead={splitReason !== null}
    aria-pressed={view.separateIndex && splitReason === null}
    aria-disabled={splitReason !== null}
    title={splitReason ?? "Separate Working Tree and Index"}
    {disabled}
    onclick={() => splitReason === null && set("separateIndex", !view.separateIndex)}
  >
    <Icon name="splitIndex" />
  </button>

  <span class="rule" aria-hidden="true"></span>

  <button
    type="button"
    class="tool"
    aria-label={layout.title}
    title={layout.title}
    {disabled}
    onclick={() => set("directories", layout.next)}
  >
    <Icon name={LAYOUT_GLYPH[layout.icon]} />
  </button>
  <!-- Slides in beside Show directories and out again; the buttons after it move with it. -->
  <span class="fold-all" class:shown={foldAll} aria-hidden={!foldAll}>
    <button
      type="button"
      class="tool"
      tabindex={foldAll ? 0 : -1}
      aria-label={foldAllTitle}
      title={foldAllTitle}
      onclick={() => filesView.toggleAll()}
    >
      <Icon name={filesView.anyCollapsed ? "expandAll" : "collapseAll"} />
    </button>
  </span>

  <span class="rule" aria-hidden="true"></span>

  {#each switches as item (item.slot)}
    <button
      type="button"
      class="tool"
      class:dead={item.reason !== null}
      aria-pressed={item.reason === null && view[item.key]}
      aria-disabled={item.reason !== null}
      aria-label={item.title}
      title={item.reason ?? item.title}
      {disabled}
      onclick={() => item.reason === null && set(item.key, !view[item.key])}
    >
      {#if item.slot === "skipped"}
        <svg class="skipped" viewBox="0 0 16 16" aria-hidden="true" shape-rendering="crispEdges"
          ><path class="page" d={GLYPHS.skipped.d} /><path class="mark" d={GLYPHS.skipped.line} /></svg
        >
      {:else}
        <FileStateIcon state={item.slot} />
      {/if}
    </button>
  {/each}

  <span class="rule" aria-hidden="true"></span>

  <div class="menu-host">
    <button
      bind:this={menuButton}
      type="button"
      class="tool"
      aria-haspopup="menu"
      aria-expanded={columnsOpen}
      title="Customize View"
      {disabled}
      onclick={toggleMenu}
    >
      <Icon name="columns" />
    </button>
    {#if columnsOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="backdrop" onclick={() => (columnsOpen = false)}></div>
      <div class="menu" role="menu" style:top="{menuAt.top}px" style:right="{menuAt.right}px">
        <p class="group-label">Columns</p>
        {#each columnMenu as column (column.key)}
          <button
            type="button"
            role="menuitemcheckbox"
            class:dead={column.reason !== null}
            aria-checked={column.checked}
            aria-disabled={column.reason !== null}
            title={column.reason ?? `Show the ${column.label} column`}
            onclick={() => column.reason === null && filesView.setColumns(toggleColumn(filesView.columns, column.key))}
          >
            <span class="tick"><MenuMark checked={column.checked} /></span>
            {column.label}
          </button>
        {/each}
        {#if context === "worktree"}
          <p class="group-label">Show files that are…</p>
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={view.renameSources}
            title={RENAME_SOURCES_TITLE}
            onclick={() => set("renameSources", !view.renameSources)}
          >
            <span class="tick"><MenuMark checked={view.renameSources} /></span>
            Rename Sources
          </button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    /* The panel header's rule: the same gaps around the field (app.css), less the 1 px a
       header keeps for its active line, which this bar has not. */
    height: calc(var(--h-panel-hdr) - 1px);
    flex: 0 0 calc(var(--h-panel-hdr) - 1px);
    padding: 0 var(--sp-3);
    border-bottom: 1px solid var(--divider);
    background: var(--surface-panel);
    overflow: hidden;
  }

  /* Gives way after the field has, down to its count and an ellipsis: the title says the rest. */
  .note {
    display: flex;
    flex: 0 1 auto;
    min-width: 0;
    line-height: 20px;
    height: 20px;
    padding: 0 var(--sp-3);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    white-space: nowrap;
  }

  .note .count {
    flex: none;
  }

  .note .words {
    min-width: 1ch;
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    /* The field narrows first, so every switch stays on the bar. */
    flex: 1 1000 auto;
    min-width: 72px;
    max-width: 260px;
    height: var(--h-panel-field);
    padding: 0 var(--sp-2) 0 var(--sp-3);
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  /* A half-typed expression is normal; the field says so without an error anywhere. */
  .field.broken {
    border-color: var(--status-delete);
  }

  .field input {
    flex: 1 1 auto;
    min-width: 0;
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
  }

  .field input:focus {
    outline: none;
  }

  .chip {
    flex: 0 0 auto;
    width: 16px;
    height: 16px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: 2px;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    line-height: 1;
    cursor: default;
  }

  .chip:disabled,
  .chip.dead {
    opacity: 0.4;
  }

  .spacer {
    flex: 1 1 auto;
    min-width: 0;
  }

  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    width: 20px;
    height: 20px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    cursor: default;
  }

  .fold-all {
    display: inline-flex;
    flex: none;
    max-width: 0;
    opacity: 0;
    overflow: hidden;
    transition:
      max-width var(--t-reveal) var(--ease-out),
      opacity var(--t-reveal) var(--ease-out);
  }

  .fold-all.shown {
    max-width: var(--h-button-sm);
    opacity: 1;
  }

  .tool {
    --file-icon: var(--panel-icon);
  }

  .tool :global(svg.icon) {
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .skipped {
    flex: none;
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
  }

  .tool .skipped .page {
    fill: var(--file-icon-page);
    stroke: var(--file-icon-stroke);
    stroke-width: 1;
    stroke-linejoin: miter;
  }

  .tool .skipped .mark {
    stroke: var(--file-icon-stroke);
    stroke-width: 1;
    shape-rendering: geometricPrecision;
  }

  /* A switch that means nothing here neither lights up nor presses (#3). */
  .tool:hover:not(:disabled, .dead) {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  /* A switch that is on keeps that look under the pointer: `[aria-pressed]` in app.css. */
  .tool[aria-pressed="true"]:hover:not(:disabled) {
    filter: brightness(1.25);
    box-shadow: inset 0 0 0 1px currentColor;
  }

  .tool[aria-pressed="true"]:active:not(:disabled) {
    filter: brightness(0.9);
  }

  .tool:disabled,
  .tool.dead {
    opacity: 0.4;
  }

  .rule {
    flex: 0 0 auto;
    width: 1px;
    height: 16px;
    background: var(--divider);
    margin-inline: var(--sp-1);
  }

  .menu-host {
    position: relative;
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: fixed;
    z-index: 31;
    min-width: 170px;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    padding: var(--sp-2) var(--sp-4);
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  .menu button:hover:not(:disabled, .dead) {
    background: var(--state-hover);
  }

  .menu button:disabled,
  .menu button.dead {
    opacity: 0.4;
  }

  .tick {
    display: inline-flex;
    width: 12px;
    color: var(--status-ref);
  }

  .group-label {
    margin: var(--sp-2) 0 var(--sp-1);
    padding: 0 var(--sp-4);
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }
</style>
