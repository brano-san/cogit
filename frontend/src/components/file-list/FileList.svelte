<script lang="ts">
  import { keyLetter } from "$lib/key-letter";
  import { modals } from "$lib/modal-stack";
  import { selectAllKeys } from "$lib/files-panel";
  import { registerSelectAll } from "$lib/select-all";
  import { untrack } from "svelte";
  import FilesToolbar from "./FilesToolbar.svelte";
  import { contentQuery, keepFile, type ContentSearch } from "$lib/content-search.svelte";
  import { compile } from "$lib/file-search";
  import type { ListContext } from "$lib/file-switches";
  import Caret from "$components/common/Caret.svelte";
  import { COLUMN_LABELS, DEFAULT_COLUMN_WIDTHS, gridColumns, nextSort, shownColumns, sortRows, type ColumnKey } from "$lib/file-columns";
  import { filesView } from "$stores/files-view.svelte";
  import { remoteOps } from "$stores/remote-ops.svelte";
  import { repository } from "$stores/repository.svelte";
  import {
    DEFAULT_VIEW,
    groupByDirectory,
    hiddenCount,
    hidingSwitches,
    paneLayout,
    shownSections,
    visibleFiles,
    type FileView,
    type ListFile,
  } from "$lib/file-view";
  import {
    actionScope,
    everyBlocked,
    requestBlocked,
    scopeBlocked,
    afterDeselect,
    applyClick,
    EMPTY_SELECTION,
    markedRows,
    rowKey,
    shownMarks,
    type FileSelection,
  } from "$lib/multi-select";
  import FilePane, { type PaneAction } from "$components/file-list/FilePane.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import type { FileEntry } from "$lib/ipc";
  import type { StateSide } from "$lib/file-state";

  interface Action {
    label: string;
    title: string;
    run: (paths: string[]) => void;
    /** Why it does not apply to one row's file (`rowActionBlocked`). */
    blocked?: (file: FileEntry) => string | null;
    /** It passes over the files it does not apply to, so the heading's "all" is off only
        when it applies to none. */
    skips?: boolean;
  }

  interface Section {
    title?: string;
    files: readonly ListFile[];
    actions?: readonly Action[];
    /** Each section can open its own side of the diff. */
    onselect?: (path: string) => void;
    /** The file this section has open in Diff, when the list's `selected` is not enough:
        a partly staged file is in two sections, and only one of them shows it. */
    selected?: string | null;
    hideWhenEmpty?: boolean;
    /** The comparison the rows come from (State column); a worktree list defaults to its
        working-tree side, any other list to a commit's. */
    side?: StateSide;
  }

  interface Props {
    sections: readonly Section[];
    /** No repository behind the list: the filter has nothing to act on. */
    disabled?: boolean;
    /** This list is in the panel holding the keyboard, so Ctrl+F is ours (issue 15). */
    activePanel?: boolean;
    selected?: string | null;
    empty?: string;
    /** The eight view switches; absent means this list is not a working tree. */
    view?: FileView;
    onview?: (view: FileView) => void;
    /** Fraction of the panel the first section keeps when the lists are shown apart. */
    split?: number;
    onsplit?: (deltaFraction: number) => void;
    onsplitreset?: () => void;
    onselect?: (path: string) => void;
    /** Double-click: open the file in its own window (T2.5). */
    onopen?: (path: string) => void;
    /** Right-click, with the title of the list the row is in (Staged means the index) and
        the rows it shows, the ones the list adds itself among them. */
    oncontext?: (path: string, event: MouseEvent, section?: string, rows?: readonly FileEntry[]) => void;
    /** Reported upward so Commit What You See knows what is hidden (T6.8). */
    onmask?: (mask: string) => void;
    /** The paths each section shows once filtered, in the order of `sections`. */
    onshown?: (shown: string[][]) => void;
    /** The ticked rows, for actions that live outside the list — stashing a selection —
        and each titled section's own, since one path can be a row of two. */
    onmarked?: (paths: string[], bySection: Record<string, string[]>) => void;
    /** Only the working tree is on disk to be searched inside. */
    contents?: ContentSearch;
    context?: ListContext;
  }

  let {
    sections,
    selected = null,
    empty,
    view,
    onview,
    split = 0.55,
    onsplit,
    onsplitreset,
    onselect,
    onopen,
    oncontext,
    onmask,
    onshown,
    onmarked,
    disabled = false,
    activePanel = false,
    contents,
    context = "worktree",
  }: Props = $props();

  const NO_HITS: ReadonlyMap<string, number> = new Map();
  const NO_MARKS: ReadonlySet<string> = new Set();

  /** Keyed by section and path (`rowKey`), not by path alone. */
  let marked = $state.raw<FileSelection>(EMPTY_SELECTION);
  let mask = $state("");
  /** The section last clicked or focused: Ctrl+A stays inside it. */
  let touched = $state<number | null>(null);
  let bar: ReturnType<typeof FilesToolbar> | undefined = $state();

  const sideOf = (section: Section): StateSide => section.side ?? (context === "worktree" ? "worktree" : "commit");

  function onkeydown(event: KeyboardEvent) {
    if (!activePanel || modals.any) return;
    if ((event.ctrlKey || event.metaKey) && keyLetter(event) === "f") {
      event.preventDefault();
      bar?.focus();
    }
  }

  // Ctrl+A ticks every file shown, in one pane (11 §4); the window-wide handler calls this.
  $effect(() => {
    if (!activePanel) return;
    return registerSelectAll("files", () => {
      const keys = selectAllKeys(groups.map((group) => group.keys), groups.findIndex((group) => group.index === touched), marked.paths);
      marked = { paths: new Set(keys), anchor: keys[0] ?? null };
    });
  });

  $effect(() => {
    onmask?.(mask);
  });

  $effect(() => {
    const bySection: Record<string, string[]> = {};
    for (const [index, paths] of marks.bySection) {
      const title = sections[index]?.title;
      if (title) bySection[title] = [...paths];
    }
    onmarked?.(marks.paths, bySection);
  });

  // The Files panel swaps one list for another; the ticks of the one that went must not
  // stay behind as the ticks of the one that came (a commit's menu acting on them).
  $effect(() => () => onmarked?.([], {}));

  let shownBefore: string | null = null;
  $effect(() => {
    const shown = selected;
    untrack(() => {
      marked = afterDeselect(shownBefore, shown, marked);
      shownBefore = shown;
    });
  });

  const active = $derived(view ?? DEFAULT_VIEW);
  const pattern = $derived(compile(mask, active.regex));
  const query = $derived(contents ? contentQuery(active, mask) : null);
  /** Until the first answer, a content search shows nothing rather than name matches. */
  const hits = $derived(query === null ? null : (contents?.hits ?? NO_HITS));

  $effect(() => {
    contents?.set(query);
  });

  $effect(() => {
    void sections;
    untrack(() => contents?.refresh());
  });

  $effect(() => () => contents?.set(null));

  const groups = $derived(
    shownSections(sections).map((section) => {
      const index = sections.indexOf(section);
      const kept = visibleFiles(section.files, active).filter((file) => keepFile(file, pattern, hits, sideOf(section)));
      const files = sortRows(kept, filesView.sort, active.directories, (path) => remoteOps.lfsState(path));
      const paths = files.map((file) => file.path);
      return {
        section,
        index,
        files,
        paths,
        keys: paths.map((path) => rowKey(index, path)),
        rows: groupByDirectory(files, active.directories),
      };
    }),
  );
  type Group = (typeof groups)[number];

  $effect(() => {
    const shown = sections.map((section) => groups.find((group) => group.section === section)?.paths ?? []);
    untrack(() => onshown?.(shown));
  });

  const layout = $derived(paneLayout(sections.length, groups.length, active.separateIndex));
  const columns = $derived(shownColumns(filesView.columns, active.directories));

  $effect(() => {
    const repo = repository.current?.repo;
    if (!repo || !columns.includes("lfs")) return;
    const paths = sections.flatMap((section) => section.files.map((file) => file.path));
    untrack(() => void remoteOps.readLfsFiles(repo, paths));
  });

  function sortLabel(key: ColumnKey): string {
    const sort = filesView.sort;
    if (sort.key !== key) return `Sort by ${COLUMN_LABELS[key]}`;
    return `Sorted by ${COLUMN_LABELS[key]}, ${sort.descending ? "descending" : "ascending"}; click to turn round`;
  }
  const apart = $derived(layout.apart);
  const total = $derived(sections.reduce((n, section) => n + section.files.length, 0));
  const shownCount = $derived(groups.reduce((n, group) => n + group.files.length, 0));
  const order = $derived(groups.flatMap((group) => group.keys));
  const came = $derived(sections.flatMap((section) => section.files));
  const hidden = $derived(hiddenCount(came, active, (file) => keepFile(file, pattern, hits)));
  const hiding = $derived(hidingSwitches(came, active));
  const visibleMarks = $derived(shownMarks(marked, order));
  const marks = $derived(markedRows(visibleMarks.paths));

  /** The rows marked in another section are not this section's to act on. */
  function scoped(group: Group, actions: readonly Action[]): PaneAction[] {
    const marked = () => marks.bySection.get(group.index) ?? NO_MARKS;
    const byPath = new Map(group.section.files.map((file) => [file.path, file]));
    return actions.map((action) => {
      const blocked = action.blocked;
      return {
        label: action.label,
        title: action.title,
        run: (request) => action.run(actionScope(marked(), request)),
        ...(blocked
          ? {
              blocked: (file: FileEntry) => scopeBlocked(marked(), file.path, byPath, blocked),
              allBlocked: (paths: readonly string[]) =>
                action.skips
                  ? everyBlocked(paths, byPath, blocked)
                  : requestBlocked(marked(), { all: paths }, byPath, blocked),
            }
          : {}),
      };
    });
  }

  function startResize(key: ColumnKey, event: PointerEvent): void {
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startWidth = filesView.widths[key] ?? DEFAULT_COLUMN_WIDTHS[key];

    function onPointerMove(e: PointerEvent) {
      const delta = e.clientX - startX;
      filesView.setWidth(key, startWidth + delta);
    }

    function onPointerUp() {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
    }

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
  }

  function clicked(group: Group, path: string, event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) {
    touched = group.index;
    marked = applyClick(marked, rowKey(group.index, path), group.keys, {
      ctrl: event.ctrlKey || event.metaKey,
      shift: event.shiftKey,
    });
    if (event.ctrlKey || event.metaKey || event.shiftKey) return;
    (group.section.onselect ?? onselect)?.(path);
  }

  function rightClicked(group: Group, path: string, event: MouseEvent) {
    touched = group.index;
    if (!marked.paths.has(rowKey(group.index, path)) && selectedIn(group) !== path) {
      clicked(group, path, { ctrlKey: false, metaKey: false, shiftKey: false });
    }
    oncontext?.(path, event, group.section.title, group.files);
  }

  function selectedIn(group: Group): string | null {
    return group.section.selected === undefined ? selected : group.section.selected;
  }

  function contentStatus(search: ContentSearch, files: number): string {
    if (search.error !== null) return search.error;
    if (search.busy) return "Searching file contents…";
    return `Found in ${files} file${files === 1 ? "" : "s"}`;
  }

  function nothingMatches(): string {
    if (query === null) return "Nothing matches the filter and the switches above.";
    return contents?.busy ? "" : "No file in this list contains the text.";
  }

  function mark(group: Group, path: string) {
    touched = group.index;
    marked = applyClick(marked, rowKey(group.index, path), group.keys, { ctrl: true, shift: false });
  }
</script>

<svelte:window {onkeydown} />

<div class="file-list">
  <FilesToolbar
    bind:this={bar}
    view={active}
    onview={(next) => onview?.(next)}
    filter={mask}
    onfilter={(text) => (mask = text)}
    {hidden}
    {hiding}
    broken={pattern.broken}
    {disabled}
    contentsReady={contents !== undefined}
    {context}
  />

  {#if query !== null && contents}
    <p class="searching" class:error={contents.error !== null} role="status">
      {contentStatus(contents, shownCount)}
    </p>
  {/if}

  {#if total === 0}
    <p class="message">{empty ?? "Nothing to show."}</p>
  {:else if shownCount === 0}
    <p class="message">{nothingMatches()}</p>
  {:else}
    <div class="table-wrap">
      <div class="columns" style:--file-grid={gridColumns(columns, filesView.widths)}>
        {#each columns as key, i (key)}
          <div class="column-header">
            <button
              type="button"
              class="column"
              class:sorted={filesView.sort.key === key}
              title={sortLabel(key)}
              aria-label={sortLabel(key)}
              onclick={() => filesView.setSort(nextSort(filesView.sort, key))}
            >
              <span class="truncate">{COLUMN_LABELS[key]}</span>
              {#if filesView.sort.key === key}<Caret open={!filesView.sort.descending} />{/if}
            </button>
            {#if i < columns.length - 1}
              <!-- svelte-ignore a11y_interactive_supports_focus -->
              <span
                class="resizer"
                role="separator"
                aria-orientation="vertical"
                onpointerdown={(event) => startResize(key, event)}
              ></span>
            {/if}
          </div>
        {/each}
      </div>
      {#if apart}
        <div class="panes">
          {#each groups as group, index (group.section.title ?? index)}
            {#if index > 0}
              <Splitter
                direction="horizontal"
                value={split}
                label="Resize the {group.section.title ?? 'file'} list"
                onchange={(delta) => onsplit?.(delta)}
                onreset={() => onsplitreset?.()}
              />
            {/if}
            <div
              class="slot"
              style:flex={index === 0 && groups.length > 1 ? `0 0 ${split * 100}%` : "1 1 auto"}
              onfocusin={() => (touched = group.index)}
            >
              <FilePane
                rows={group.rows}
                title={group.section.title}
                paths={group.paths}
                actions={scoped(group, group.section.actions ?? [])}
                {columns}
                nested={active.directories}
                side={sideOf(group.section)}
                selected={selectedIn(group)}
                marked={marks.bySection.get(group.index) ?? NO_MARKS}
                onclick={(path, event) => clicked(group, path, event)}
                onmark={(path) => mark(group, path)}
                {onopen}
                oncontext={oncontext && ((path, event) => rightClicked(group, path, event))}
              />
            </div>
          {/each}
        </div>
      {:else}
        <div class="panes">
          {#each groups as group, index (group.section.title ?? index)}
            {#if group.files.length > 0}
              <div class="slot grow" onfocusin={() => (touched = group.index)}>
                <FilePane
                  rows={group.rows}
                  title={layout.titled ? group.section.title : undefined}
                  paths={group.paths}
                  actions={scoped(group, group.section.actions ?? [])}
                  {columns}
                  nested={active.directories}
                  side={sideOf(group.section)}
                  selected={selectedIn(group)}
                  marked={marks.bySection.get(group.index) ?? NO_MARKS}
                  onclick={(path, event) => clicked(group, path, event)}
                  onmark={(path) => mark(group, path)}
                  {onopen}
                  oncontext={oncontext && ((path, event) => rightClicked(group, path, event))}
                />
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .file-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .table-wrap {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    overflow-x: auto;
    overflow-y: hidden;
  }

  /* In line with the rows of FilePane: the same grid, gap and padding, and the room of the
     list's scrollbar, which FilePane keeps whether it shows or not (#33). */
  .columns {
    display: grid;
    grid-template-columns: var(--file-grid);
    column-gap: var(--file-column-gap);
    flex: 0 0 auto;
    height: var(--h-row-dense);
    padding: 0 calc(var(--sp-5) + var(--scrollbar-size)) 0 var(--sp-5);
    border-bottom: 1px solid var(--divider);
    min-width: max-content;
  }

  .column-header {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 0;
    height: 100%;
  }

  .column {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--fs-header);
    text-align: left;
    cursor: default;
  }

  .resizer {
    position: absolute;
    right: calc(-1 * var(--file-column-gap) / 2 - 3px);
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 2;
    touch-action: none;
  }

  .resizer::after {
    content: "";
    position: absolute;
    left: 2px;
    top: 4px;
    bottom: 4px;
    width: 1px;
    background: var(--divider);
    opacity: 0.8;
  }

  .resizer:hover::after {
    background: var(--status-ref);
    opacity: 1;
    width: 2px;
  }

  .column:hover,
  .column.sorted {
    color: var(--text-primary);
  }

  .panes {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    min-width: max-content;
  }

  .slot {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .slot.grow {
    flex: 1 1 auto;
  }

  .searching {
    flex: 0 0 auto;
    margin: 0;
    padding: var(--sp-1) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .searching.error {
    color: var(--status-delete);
    user-select: text;
  }

  .message {
    margin: 0;
    padding: var(--sp-7) var(--sp-5);
    text-align: center;
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }
</style>
