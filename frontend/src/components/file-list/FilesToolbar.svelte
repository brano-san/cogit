<script lang="ts">
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
    /** How many rows the switches and the filter are keeping out of sight. */
    hidden: number;
    /** The switches among them that hid rows; only these go on to show them. */
    hiding?: readonly (keyof FileView)[];
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
    hiding = [],
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
  let bar: HTMLDivElement | undefined = $state();
  let crowded = $state(false);

  /** Field at its narrowest, plus the eight buttons and three rules to its right. */
  const ROOM_FOR_SWITCHES = 420;

  /** The Files panel is often a narrow column. Rather than clip the switches, they move
      into the Customise View menu, where they are still one click away (issue 12). */
  $effect(() => {
    const element = bar;
    if (!element) return;
    // Read the switches so a change in what is rendered re-measures the row.
    void hidden;
    void view;

    // A width, not a measurement of the overflow: once the switches are hidden the row
    // fits again, so measuring the overflow makes the decision flap.
    const measure = () => {
      crowded = element.clientWidth < ROOM_FOR_SWITCHES;
    };
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    // One frame later: on the first pass the children have no laid-out width yet.
    const frame = requestAnimationFrame(measure);
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  });

  export function focus() {
    box?.select();
  }

  const set = (key: keyof FileView, value: boolean) => onview({ ...view, [key]: value });

  /** Lucide paths, the family the main toolbar uses. */
  const I = {
    search: "M11 5a6 6 0 1 0 0 12 6 6 0 0 0 0-12m9 15-4.3-4.3",
    split: "M4 4h10v10H4zM10 10h10v10H10m2-2 2 2 4-4",
    tree: "M3 6.5A1.5 1.5 0 0 1 4.5 5h3l1.5 2h10a1.5 1.5 0 0 1 1.5 1.5v9A1.5 1.5 0 0 1 19 19H4.5A1.5 1.5 0 0 1 3 17.5ZM9 11v5m0-5h4m-4 5h4",
    flat: "M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01",
    unchanged: "M6 3h8l5 5v13H6zM14 3v5h5",
    untracked: "M6 3h8l5 5v13H6zM14 3v5h5M9 12h6m-3-3v6",
    ignored: "M6 3h8l5 5v13H6zM14 3v5h5M9 11l6 6m0-6-6 6",
    modified: "M6 3h8l5 5v13H6zM14 3v5h5M9 15l6-6",
    skipped: "M6 3h8l5 5v13H6zM14 3v5h5M10 11v6M14 11v6",
    missing: "M6 3h8l5 5v13H6zM14 3v5h5M9 14h6",
    columns: "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2ZM9 3v18M15 3v18",
  } as const;

  const switches = $derived(stateSwitches(context));
  const layout = $derived(layoutToggle(view.directories));
  const splitReason = $derived(toolReason(context, "separateIndex"));
  const contentsReason = $derived(
    toolReason(context, "contents") ??
      (contentsReady ? null : "Search in file contents — only the working tree is on disk to search"),
  );

  /** Customise View (#34): the columns of the table, then what the bar has no room for. On a
      commit the sources of renames are the Missing switch; the working tree has them here. */
  const columnMenu = $derived(columnItems(filesView.columns, view.directories));

  /** The hidden rows come back: the filter text goes and the switches that hid them go on.
      The rest stay as they are — Unchanged or Ignored would read the whole tree. */
  function showHidden() {
    onfilter("");
    const live = new Set(switches.filter((item) => item.reason === null).map((item) => item.key));
    const turned = hiding.filter((key) => live.has(key) && !view[key]);
    if (turned.length === 0) return;
    const next = { ...view };
    for (const key of turned) next[key] = true;
    onview(next);
  }
</script>

<div class="bar" class:crowded bind:this={bar} role="toolbar" aria-label="File list options">
  {#if hidden > 0}
    <button
      type="button"
      class="badge"
      title="Show the hidden files: clear the filter and turn on the switches hiding them"
      onclick={showHidden}
    >
      ✕ {hidden} file{hidden === 1 ? "" : "s"} hidden
    </button>
  {/if}

  <div class="field" class:broken>
    <svg class="lens" viewBox="0 0 24 24" aria-hidden="true"><path d={I.search} /></svg>
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
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d={I.split} /></svg>
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
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d={I[layout.icon]} /></svg>
  </button>

  <span class="rule" aria-hidden="true"></span>

  {#each crowded ? [] : switches as item (item.slot)}
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
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d={I[item.slot]} /></svg>
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
      title="Customise View"
      {disabled}
      onclick={toggleMenu}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d={I.columns} /></svg>
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
            <span class="tick">{column.checked ? "✓" : ""}</span>
            {column.label}
          </button>
        {/each}
        {#if crowded || context === "worktree"}
          <p class="group-label">Show files that are…</p>
        {/if}
        {#if crowded}
          {#each switches as item (item.slot)}
            <button
              type="button"
              role="menuitemcheckbox"
              class:dead={item.reason !== null}
              aria-checked={item.reason === null && view[item.key]}
              aria-disabled={item.reason !== null}
              title={item.reason ?? item.title}
              onclick={() => item.reason === null && set(item.key, !view[item.key])}
            >
              <span class="tick">{item.reason === null && view[item.key] ? "✓" : ""}</span>
              {item.key === "renameSources" ? "Rename Sources" : item.slot.charAt(0).toUpperCase() + item.slot.slice(1)}
            </button>
          {/each}
        {/if}
        {#if context === "worktree"}
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={view.renameSources}
            title={RENAME_SOURCES_TITLE}
            onclick={() => set("renameSources", !view.renameSources)}
          >
            <span class="tick">{view.renameSources ? "✓" : ""}</span>
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
    height: 32px;
    flex: 0 0 32px;
    padding: 0 var(--sp-3);
    border-bottom: 1px solid var(--divider);
    background: var(--surface-panel);
    overflow: hidden;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    height: 20px;
    padding: 0 var(--sp-3);
    background: var(--state-hover);
    color: var(--text-secondary);
    border: 0;
    border-radius: var(--r-sm);
    font-size: var(--fs-header);
    white-space: nowrap;
    cursor: default;
  }

  .badge:hover {
    color: var(--text-primary);
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    flex: 1 1 auto;
    min-width: 90px;
    max-width: 260px;
    height: 22px;
    padding: 0 var(--sp-2);
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  /* A half-typed expression is normal; the field says so without an error anywhere. */
  .field.broken {
    border-color: var(--status-delete);
  }

  .lens {
    flex: 0 0 auto;
    width: 12px;
    height: 12px;
    fill: none;
    stroke: var(--text-secondary);
    stroke-width: 2;
    stroke-linecap: round;
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

  .tool svg {
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  /* A switch that means nothing here neither lights up nor presses (#3). */
  .tool:hover:not(:disabled, .dead) {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  /* A switch that is on keeps that look under the pointer: `[aria-pressed]` in app.css. */
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
    display: inline-block;
    width: 10px;
    color: var(--status-ref);
  }

  .group-label {
    margin: var(--sp-2) 0 var(--sp-1);
    padding: 0 var(--sp-4);
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  /* Once the switches have moved into the menu, the rules that fenced them go too. */
  .bar.crowded .rule:nth-of-type(n + 2) {
    display: none;
  }
</style>
