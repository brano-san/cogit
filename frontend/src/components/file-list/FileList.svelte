<script lang="ts">
  import { matchesMask, sortFiles } from "$lib/files";
  import type { SortKey } from "$lib/files";
  import {
    DEFAULT_VIEW,
    TOGGLES,
    groupByDirectory,
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
  }

  interface Props {
    sections: readonly Section[];
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
    onmask,
    onmarked,
  }: Props = $props();

  const SORTS: { key: SortKey; label: string }[] = [
    { key: "path", label: "Path" },
    { key: "name", label: "Name" },
    { key: "status", label: "Status" },
  ];

  let marked = $state.raw<FileSelection>(EMPTY_SELECTION);
  let mask = $state("");
  let sort = $state<SortKey>("path");

  $effect(() => {
    onmask?.(mask);
  });

  $effect(() => {
    onmarked?.([...marked.paths]);
  });

  const active = $derived(view ?? DEFAULT_VIEW);
  const groups = $derived(
    sections.map((section) => {
      const files = sortFiles(
        visibleFiles(section.files, active).filter((file) => matchesMask(file.path, mask)),
        sort,
      );
      return {
        section,
        files,
        paths: files.map((file) => file.path),
        rows: groupByDirectory(files, active.directories),
      };
    }),
  );

  /** One pane per section only when there is a second one to put beside it. */
  const apart = $derived(active.separateIndex && groups.length > 1);
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

<div class="file-list">
  <div class="controls">
    <input
      class="mask"
      type="search"
      bind:value={mask}
      placeholder="Filter, e.g. *.rs"
      aria-label="Filter files by mask"
    />
    <div class="sort" role="group" aria-label="Sort files">
      {#each SORTS as option (option.key)}
        <button
          type="button"
          class:active={sort === option.key}
          title="Sort by {option.label.toLowerCase()}"
          onclick={() => (sort = option.key)}>{option.label}</button
        >
      {/each}
    </div>
  </div>

  {#if view}
    <div class="toggles" role="group" aria-label="What the list shows">
      {#each TOGGLES as item (item.key)}
        <button
          type="button"
          class:on={active[item.key]}
          aria-pressed={active[item.key]}
          aria-label={item.title}
          title={item.title}
          onclick={() => onview?.({ ...active, [item.key]: !active[item.key] })}
          >{item.icon}</button
        >
      {/each}
    </div>
  {/if}

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
        <div class="slot" style:flex={index === 0 ? `0 0 ${split * 100}%` : "1 1 auto"}>
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
              title={groups.length > 1 ? group.section.title : undefined}
              paths={group.paths}
              actions={scoped(group.section.actions ?? [])}
              showDirectory={!active.directories}
              {selected}
              marked={marked.paths}
              onclick={(path, event) => clicked(group.section, path, event)}
              onmark={mark}
              {onopen}
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

  .controls {
    display: flex;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
  }

  .mask {
    flex: 1 1 auto;
    min-width: 0;
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  /* All three choices stay visible: a dropdown hides the two the user is not on. */
  .sort {
    display: flex;
    flex: 0 0 auto;
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    overflow: hidden;
  }

  .sort button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-secondary);
    border: 0;
    border-left: 1px solid var(--field-border);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  .sort button:first-child {
    border-left: 0;
  }

  .sort button:hover {
    color: var(--text-primary);
  }

  .sort button.active {
    background: var(--state-selected);
    color: var(--text-primary);
  }

  .toggles {
    display: flex;
    gap: var(--sp-1);
    flex: 0 0 auto;
    padding: var(--sp-2) var(--sp-4);
    border-bottom: 1px solid var(--divider);
  }

  .toggles button {
    width: 22px;
    height: var(--h-button-sm);
    background: transparent;
    color: var(--text-secondary);
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    line-height: 1;
    cursor: default;
  }

  .toggles button:hover {
    border-color: var(--field-border);
    color: var(--text-primary);
  }

  .toggles button.on {
    background: var(--state-selected);
    border-color: var(--field-border);
    color: var(--status-ref);
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
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }
</style>
