<script lang="ts">
  import { keyLetter } from "$lib/key-letter";
  import { modals } from "$lib/modal-stack";
  import { untrack } from "svelte";
  import FilesToolbar from "./FilesToolbar.svelte";
  import { contentQuery, keepFile, type ContentSearch } from "$lib/content-search.svelte";
  import { compile } from "$lib/file-search";
  import type { ListContext } from "$lib/file-switches";
  import { sortFiles } from "$lib/files";
  import {
    DEFAULT_VIEW,
    groupByDirectory,
    hidingSwitches,
    paneLayout,
    shownSections,
    visibleFiles,
    type FileView,
  } from "$lib/file-view";
  import { afterDeselect, applyClick, EMPTY_SELECTION, shownMarks, type FileSelection } from "$lib/multi-select";
  import FilePane from "$components/file-list/FilePane.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import type { FileEntry } from "$lib/ipc";

  interface Action {
    label: string;
    title: string;
    run: (paths: string[]) => void;
  }

  interface Section {
    title?: string;
    files: readonly FileEntry[];
    actions?: readonly Action[];
    /** Each section can open its own side of the diff. */
    onselect?: (path: string) => void;
    hideWhenEmpty?: boolean;
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
    /** Right-click, with the title of the list the row is in: Staged means the index. */
    oncontext?: (path: string, event: MouseEvent, section?: string) => void;
    /** Reported upward so Commit What You See knows what is hidden (T6.8). */
    onmask?: (mask: string) => void;
    /** The paths each section shows once filtered, in the order of `sections`. */
    onshown?: (shown: string[][]) => void;
    /** The ticked rows, for actions that live outside the list — stashing a selection. */
    onmarked?: (paths: string[]) => void;
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

  let marked = $state.raw<FileSelection>(EMPTY_SELECTION);
  let mask = $state("");
  let bar: ReturnType<typeof FilesToolbar> | undefined = $state();

  function onkeydown(event: KeyboardEvent) {
    if (!activePanel || modals.any) return;
    // Ctrl+A ticks every file shown (11 §4); in a field it selects the text.
    if ((event.ctrlKey || event.metaKey) && event.code === "KeyA" && !typingIn(event.target)) {
      event.preventDefault();
      marked = { paths: new Set(order), anchor: order[0] ?? null };
      return;
    }
    if ((event.ctrlKey || event.metaKey) && keyLetter(event) === "f") {
      event.preventDefault();
      bar?.focus();
    }
  }

  $effect(() => {
    onmask?.(mask);
  });

  $effect(() => {
    onmarked?.([...visibleMarks.paths]);
  });

  // The Files panel swaps one list for another; the ticks of the one that went must not
  // stay behind as the ticks of the one that came (a commit's menu acting on them).
  $effect(() => () => onmarked?.([]));

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
      const files = sortFiles(
        visibleFiles(section.files, active).filter((file) => keepFile(file, pattern, hits)),
        "path",
      );
      return {
        section,
        files,
        paths: files.map((file) => file.path),
        rows: groupByDirectory(files, active.directories),
      };
    }),
  );

  $effect(() => {
    const shown = sections.map((section) => groups.find((group) => group.section === section)?.paths ?? []);
    untrack(() => onshown?.(shown));
  });

  const layout = $derived(paneLayout(sections.length, groups.length, active.separateIndex));
  const apart = $derived(layout.apart);
  const total = $derived(sections.reduce((n, section) => n + section.files.length, 0));
  const shownCount = $derived(groups.reduce((n, group) => n + group.files.length, 0));
  const order = $derived(groups.flatMap((group) => group.paths));
  const hiding = $derived(hidingSwitches(sections.flatMap((section) => section.files), active));
  const visibleMarks = $derived(shownMarks(marked, order));

  /** The paths an action applies to: the marked set when the clicked file is in it. */
  function scopeOf(path: string): string[] {
    const paths = visibleMarks.paths;
    return paths.has(path) && paths.size > 1 ? [...paths] : [path];
  }

  function scoped(actions: readonly Action[]): Action[] {
    return actions.map((action) => ({
      ...action,
      run: (paths: string[]) => action.run(paths.length === 1 ? scopeOf(paths[0] ?? "") : paths),
    }));
  }

  function clicked(section: Section, path: string, event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) {
    marked = applyClick(marked, path, order, {
      ctrl: event.ctrlKey || event.metaKey,
      shift: event.shiftKey,
    });
    if (event.ctrlKey || event.metaKey || event.shiftKey) return;
    (section.onselect ?? onselect)?.(path);
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

  function typingIn(target: EventTarget | null): boolean {
    return target instanceof HTMLTextAreaElement || (target instanceof HTMLInputElement && target.type !== "checkbox");
  }

  function mark(path: string) {
    marked = applyClick(marked, path, order, { ctrl: true, shift: false });
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
    hidden={total - shownCount}
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
  {:else if apart}
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
        <div class="slot" style:flex={index === 0 && groups.length > 1 ? `0 0 ${split * 100}%` : "1 1 auto"}>
          <FilePane
            rows={group.rows}
            title={group.section.title}
            paths={group.paths}
            actions={scoped(group.section.actions ?? [])}
            showDirectory={!active.directories}
            {selected}
            marked={visibleMarks.paths}
            onclick={(path, event) => clicked(group.section, path, event)}
            onmark={mark}
            {onopen}
            oncontext={oncontext && ((path, event) => oncontext(path, event, group.section.title))}
          />
        </div>
      {/each}
    </div>
  {:else}
    <div class="panes">
      {#each groups as group, index (group.section.title ?? index)}
        {#if group.files.length > 0}
          <div class="slot grow">
            <FilePane
              rows={group.rows}
              title={layout.titled ? group.section.title : undefined}
              paths={group.paths}
              actions={scoped(group.section.actions ?? [])}
              showDirectory={!active.directories}
              {selected}
              marked={visibleMarks.paths}
              onclick={(path, event) => clicked(group.section, path, event)}
              onmark={mark}
              {onopen}
              oncontext={oncontext && ((path, event) => oncontext(path, event, group.section.title))}
            />
          </div>
        {/if}
      {/each}
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

  .panes {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
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
