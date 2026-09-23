<script lang="ts">
  import FilesToolbar from "./FilesToolbar.svelte";
  import { compile, matches } from "$lib/file-search";
  import { sortFiles } from "$lib/files";
  import {
    DEFAULT_VIEW,
    groupByDirectory,
    paneLayout,
    shownSections,
    visibleFiles,
    type FileView,
  } from "$lib/file-view";
  import { applyClick, EMPTY_SELECTION, type FileSelection } from "$lib/multi-select";
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
    /** Right-click: the row becomes the selection, then the menu opens on it. */
    oncontext?: (path: string, event: MouseEvent) => void;
    /** Reported upward so Commit What You See knows what is hidden (T6.8). */
    onmask?: (mask: string) => void;
    /** The ticked rows, for actions that live outside the list — stashing a selection. */
    onmarked?: (paths: string[]) => void;
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
    onmarked,
    disabled = false,
    activePanel = false,
  }: Props = $props();

  let marked = $state.raw<FileSelection>(EMPTY_SELECTION);
  let mask = $state("");
  let bar: ReturnType<typeof FilesToolbar> | undefined = $state();

  function onkeydown(event: KeyboardEvent) {
    if (!activePanel) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      bar?.focus();
    }
  }

  $effect(() => {
    onmask?.(mask);
  });

  $effect(() => {
    onmarked?.([...marked.paths]);
  });

  const active = $derived(view ?? DEFAULT_VIEW);
  const pattern = $derived(compile(mask, active.regex));
  const groups = $derived(
    shownSections(sections).map((section) => {
      const files = sortFiles(
        visibleFiles(section.files, active).filter((file) => matches(file, pattern)),
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

  const layout = $derived(paneLayout(sections.length, groups.length, active.separateIndex));
  const apart = $derived(layout.apart);
  const total = $derived(sections.reduce((n, section) => n + section.files.length, 0));
  const shownCount = $derived(groups.reduce((n, group) => n + group.files.length, 0));
  const order = $derived(groups.flatMap((group) => group.paths));

  /** The paths an action applies to: the marked set when the clicked file is in it. */
  function scopeOf(path: string): string[] {
    return marked.paths.has(path) && marked.paths.size > 1 ? [...marked.paths] : [path];
  }

  function scoped(actions: readonly Action[]): Action[] {
    return actions.map((action) => ({
      ...action,
      run: (paths: string[]) => action.run(paths.length === 1 ? scopeOf(paths[0] ?? "") : paths),
    }));
  }

  function clicked(section: Section, path: string, event: MouseEvent) {
    marked = applyClick(marked, path, order, {
      ctrl: event.ctrlKey || event.metaKey,
      shift: event.shiftKey,
    });
    if (event.ctrlKey || event.metaKey || event.shiftKey) return;
    (section.onselect ?? onselect)?.(path);
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
    broken={pattern.broken}
    {disabled}
  />

  {#if total === 0}
    <p class="message">{empty ?? "Nothing to show."}</p>
  {:else if shownCount === 0}
    <p class="message">Nothing matches the filter and the switches above.</p>
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
            marked={marked.paths}
            onclick={(path, event) => clicked(group.section, path, event)}
            onmark={mark}
            {onopen}
            {oncontext}
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
              marked={marked.paths}
              onclick={(path, event) => clicked(group.section, path, event)}
              onmark={mark}
              {onopen}
              {oncontext}
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

  .message {
    margin: 0;
    padding: var(--sp-7) var(--sp-5);
    text-align: center;
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }
</style>
