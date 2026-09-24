<script lang="ts">
  import Disclosure from "$components/common/Disclosure.svelte";
  import KindIcon, { type Kind } from "$components/common/KindIcon.svelte";
  import VirtualList from "$components/common/VirtualList.svelte";
  import { fileName, statusBadge, statusLabel, statusTooltip } from "$lib/files";
  import type { FileEntry } from "$lib/ipc";
  import type { ViewRow } from "$lib/file-view";
  import { LIST_ROW_HEIGHT } from "$lib/graph-geometry";

  interface Action {
    label: string;
    title: string;
    run: (paths: string[]) => void;
  }

  interface Props {
    rows: readonly ViewRow[];
    title?: string;
    selected?: string | null;
    marked: ReadonlySet<string>;
    actions?: readonly Action[];
    /** Paths shown here, for the "all" buttons in the heading. */
    paths: readonly string[];
    /** Full paths are redundant once the list groups by directory. */
    showDirectory?: boolean;
    onclick: (path: string, event: MouseEvent) => void;
    onmark: (path: string) => void;
    onopen?: (path: string) => void;
    oncontext?: (path: string, event: MouseEvent) => void;
  }

  let {
    rows,
    title,
    selected = null,
    marked,
    actions = [],
    paths,
    showDirectory = true,
    onclick,
    onmark,
    onopen,
    oncontext,
  }: Props = $props();

  /** By the entry's mode, 160000 and 120000, never by the name (R-180). */
  function kindOf(file: FileEntry): Kind {
    if (file.mode === "submodule") return "submodule";
    if (file.mode === "symlink") return "symlink";
    return file.path.endsWith("/") ? "directory" : "file";
  }

  function directory(path: string): string {
    const cut = path.lastIndexOf("/");
    return cut === -1 ? "" : path.slice(0, cut + 1);
  }
</script>

<div class="pane tree-rows key-list">
  {#if title}
    <div class="heading">
      <span class="grow">{title} ({paths.length})</span>
      {#each actions as action (action.label)}
        <button type="button" class="act" title="{action.title} — all" onclick={() => action.run([...paths])}
          >{action.label} all</button
        >
      {/each}
    </div>
  {/if}

  <VirtualList items={rows} label={title ?? "Files"}>
    {#snippet row(entry, at)}
        {#if entry.kind === "dir"}
          {@const group = entry}
          <div class="folder" style:top="{at * LIST_ROW_HEIGHT}px">
            <Disclosure open />
            <span class="truncate">{group.path === "" ? "(root)" : group.path}</span>
            <span class="count">{group.count}</span>
          </div>
        {:else}
          {@const file = entry.file}
          <button
            type="button"
            class="row {file.status}"
            class:nested={!showDirectory}
            class:selected={selected === file.path}
            class:marked={marked.has(file.path)}
            style:top="{at * LIST_ROW_HEIGHT}px"
            title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
            onclick={(event) => onclick(file.path, event)}
            onkeydown={(event) => {
              if (event.key !== " ") return;
              event.preventDefault();
              onmark(file.path);
            }}
            ondblclick={() => onopen?.(file.path)}
            oncontextmenu={(event) => {
              if (!oncontext) return;
              event.preventDefault();
              oncontext(file.path, event);
            }}
          >
            <KindIcon kind={kindOf(file)} />
            <span class="badge" aria-label={statusLabel(file.status)} title={statusTooltip(file.status)}
              >{statusBadge(file.status)}</span
            >
            <span class="name truncate shrink-last">{fileName(file.path)}</span>
            {#if file.oldPath}
              <span class="renamed truncate shrink-first" title="from {file.oldPath}"
                >← {fileName(file.oldPath)}{file.similarity !== null
                  ? ` ${file.similarity}%`
                  : ""}</span
              >
            {/if}
            {#if file.modeChange}
              <span class="mode" title="Mode changed to {file.modeChange}"
                >{file.modeChange === "executable" ? "+x" : "−x"}</span
              >
            {/if}
            <span class="dir truncate shrink-first">{showDirectory ? directory(file.path) : ""}</span>
            {#each actions as action (action.label)}
              <span
                class="act"
                role="button"
                tabindex="-1"
                title={action.title}
                onclick={(event) => {
                  event.stopPropagation();
                  action.run([file.path]);
                }}
                onkeydown={(event) => {
                  if (event.key === "Enter") action.run([file.path]);
                }}>{action.label}</span
              >
            {/each}
          </button>
        {/if}
    {/snippet}
  </VirtualList>
</div>

<style>
  .pane {
    --tree-gap: var(--sp-3);
    --tree-next: var(--disclosure-glyph);
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  .heading {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .row,
  .folder {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--tree-gap);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  /* Grouped by directory, a file is one level in: its icon where a child's triangle goes. */
  .folder {
    padding-left: var(--tree-base);
  }

  .row.nested {
    padding-left: calc(
      var(--tree-base) + var(--tree-step) + (var(--disclosure-glyph) - var(--kind-icon)) / 2
    );
  }

  .row {
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.marked {
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  /* A bar as well as a tint: two greys apart is not something everyone can see. */
  .row.selected {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .folder {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .folder .count {
    margin-left: auto;
  }

  .grow {
    flex: 1 1 auto;
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0;
    cursor: default;
  }

  .row:hover .act,
  .heading:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-ref);
  }

  /* Dimmed: it is context for the name, not a thing to read on its own. */
  .badge {
    flex: 0 0 auto;
    width: 12px;
    font-family: var(--font-mono);
    font-weight: 600;
    text-align: center;
  }

  .row.added .badge {
    color: var(--status-add);
  }

  .row.modified .badge {
    color: var(--status-modify);
  }

  .row.deleted .badge {
    color: var(--status-delete);
  }

  .row.renamed .badge,
  .row.copied .badge {
    color: var(--status-ref);
  }

  .row.untracked .badge,
  .row.unchanged .badge,
  .row.ignored .badge,
  .row.assumeUnchanged .badge,
  .row.skipped .badge {
    color: var(--text-secondary);
  }

  .row.unchanged,
  .row.ignored,
  .row.assumeUnchanged,
  .row.skipped {
    color: var(--text-secondary);
  }

  .row.conflicted .badge {
    color: var(--status-delete);
    background: var(--c-deleted-bg);
  }

  .renamed {
    color: var(--status-ref);
    font-size: 10px;
  }

  .mode {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .dir {
    flex-grow: 1;
    color: var(--text-secondary);
    font-size: 11px;
  }
</style>
