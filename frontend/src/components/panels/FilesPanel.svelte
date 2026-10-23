<script lang="ts">
  import { untrack } from "svelte";
  import FileList from "$components/file-list/FileList.svelte";
  import { ContentSearch } from "$lib/content-search.svelte";
  import { rowActionBlocked, type RowAction } from "$lib/file-menu";
  import { withUnchanged } from "$lib/file-switches";
  import { emptyText, filesPanelList, mergedSide, mergeIndex, pairMemo, searchesEveryFile, stagedShown } from "$lib/files-panel";
  import type { FileView } from "$lib/file-view";
  import type { FileEntry } from "$lib/ipc";
  import { effective, shortcutOf } from "$lib/keymap";
  import { ON_MAC } from "$lib/platform";
  import { settings } from "$stores/settings.svelte";
  import { commit } from "$stores/commit.svelte";
  import { commitTree } from "$stores/commit-tree.svelte";
  import { compareView } from "$stores/compare-view.svelte";
  import { diff } from "$stores/diff.svelte";
  import { errors } from "$stores/errors.svelte";
  import { fileTotal } from "$stores/file-total.svelte";
  import { filesView } from "$stores/files-view.svelte";
  import { layout } from "$stores/layout.svelte";
  import { repository } from "$stores/repository.svelte";
  import { stashView } from "$stores/stash-view.svelte";
  import { worktree } from "$stores/worktree.svelte";

  /** Two lists in one place: the working tree while nothing is selected, and the files a
      selected commit changed. They share the panel but almost nothing else. */
  interface Props {
    /** False while no repository is open: the list has nothing behind it at all. */
    view: import("$lib/repo-phase").PanelView;
    /** This panel holds the keyboard (issue 15). */
    activePanel: boolean;
    onWorkingTree: boolean;
    onviewchange: (next: FileView) => void;
    onopenworktree: (path: string) => void;
    onopenstaged: (path: string) => void;
    onopencommit: (path: string) => void;
    /** A stash file names which of the three parts it came from. */
    onopenstash: (part: "worktree" | "index" | "untracked", path: string) => void;
    /** A file of the comparison (Compare with HEAD / with Selected Commit). */
    onopencompare: (path: string) => void;
    onopenwindow: (path: string) => void;
    /** Double-click or Enter on a Working Tree row: Unstaged or Staged, whatever Diff shows. */
    onopenrow: (path: string, side: "worktree" | "index") => void;
    /** Double-click or Enter on a row of a past commit. */
    onopenpast: (path: string) => void;
    onmask: (mask: string) => void;
    /** The staged files the working-tree list shows once filtered. */
    onshownstaged?: (paths: string[] | null) => void;
    onmarked: (paths: string[], bySection: Record<string, string[]>) => void;
    oncontext: (path: string, event: MouseEvent, section?: string, rows?: readonly FileEntry[]) => void;
    stage: (paths: string[]) => void;
    stagemode: (paths: string[]) => void;
    unstage: (paths: string[]) => void;
    discard: (paths: string[]) => void;
    ignore: (paths: string[]) => void;
    remove: (paths: string[]) => void;
    /** The rows of the list on screen, for the panel header. */
    oncount?: (count: number | undefined) => void;
  }

  let {
    view,
    activePanel,
    onWorkingTree,
    onviewchange,
    onopenworktree,
    onopenstaged,
    onopencommit,
    onopenstash,
    onopencompare,
    onopenwindow,
    onopenrow,
    onopenpast,
    onmask,
    onshownstaged,
    onmarked,
    oncontext,
    stage,
    stagemode,
    unstage,
    discard,
    ignore,
    remove,
    oncount,
  }: Props = $props();

  const keys = $derived(effective(settings.bindings, settings.keymap));
  /** Ctrl+T / Ctrl+Shift+T act on the ticked files, or on all of them like these buttons. */
  const shortcut = (command: string) => shortcutOf(command, keys, ON_MAC);

  const fractions = $derived(layout.fractions);
  const contents = new ContentSearch(() => repository.current?.repo ?? null);

  /** A commit's whole tree is read only while its Unchanged switch asks for it. */
  $effect(() => {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (!onWorkingTree && filesView.commit.unchanged && id && oid) void commitTree.load(id, oid);
  });

  $effect(() => errors.report(commitTree.error, "Could not list the unchanged files of the commit"));

  const commitList = $derived(
    withUnchanged(
      commit.files,
      filesView.commit.unchanged && commitTree.oid === commit.oid ? commitTree.paths : null,
    ),
  );

  const shown = $derived(
    filesPanelList({
      content: view === "content",
      stash: stashView.contents,
      compare: compareView.showing(commit.oid) ? compareView.files : null,
      onWorkingTree,
      worktree: worktree.total,
      commit: commitList.length,
    }),
  );

  $effect(() => oncount?.(shown.count));

  /** The revision the list is of, for "N files hidden"; the index is read again whenever the
      working tree's lists are. */
  $effect(() => {
    const repo = repository.current?.repo;
    const kind = shown.kind;
    const rev =
      kind === "commit"
        ? commit.oid
        : kind === "stash"
          ? (stashView.contents?.worktreeRev ?? null)
          : kind === "compare"
            ? compareView.to
            : null;
    if (kind === "worktree") void [worktree.staged, worktree.unstaged];
    untrack(() => {
      if (repo === undefined || kind === "none" || (kind !== "worktree" && rev === null)) fileTotal.clear();
      else void fileTotal.read(repo, rev);
    });
  });

  /** The working tree list's filter: over a clean tree it looks through every file. Another
      list in the panel has a filter of its own, so this one starts empty again. */
  let mask = $state("");
  $effect(() => {
    if (shown.kind !== "worktree") mask = "";
  });
  const everyFile = $derived(
    shown.kind === "worktree" && !filesView.current.unchanged && searchesEveryFile(mask, repository.current?.status),
  );
  $effect(() => {
    const on = everyFile;
    const id = repository.current?.repo ?? null;
    untrack(() => void worktree.searchEveryFile(on, id));
  });
  $effect(() => () => void worktree.searchEveryFile(false, null));
  /** What the search found is listed with the changes, never counted or staged as one. */
  const withFound = pairMemo((found: FileEntry[], unstaged: FileEntry[]) => [...unstaged, ...found]);
  const unstagedRows = $derived(
    worktree.found.length > 0 ? withFound(worktree.found, worktree.unstaged) : worktree.unstaged,
  );
  const mergedRows = pairMemo(mergeIndex);

  /** Unstaged and Staged, or one list of both while the index is not shown apart (#32). */
  const separate = $derived(filesView.current.separateIndex);
  const merged = $derived(separate ? [] : mergedRows(unstagedRows, worktree.staged));

  /** A row button off where the Files menu has its item off (#40). */
  const rowBlocked = (action: RowAction) => (file: FileEntry) => rowActionBlocked(action, file);

  /** Three different nothings, and the panel used to say the same thing for all of them. */
  const nothing = $derived(
    view !== "content"
      ? ""
      : commit.oid === null
        ? "Select a commit to see the files it changed."
        : emptyText({ settled: !commit.loading, failed: commit.error !== null }, "This commit changed no files."),
  );
</script>

<div class="files">
  {#if shown.kind === "none"}
    <FileList sections={[{ files: [] }]} empty={nothing} disabled {activePanel} />
  {:else if shown.kind === "stash" && stashView.contents}
    {@const parts = stashView.contents}
    <FileList
      {activePanel}
      revisionFiles={fileTotal.value}
      context="stash"
      view={filesView.commit}
      onview={(next) => filesView.setCommit(next)}
      sections={[
        {
          title: "Working tree",
          files: parts.worktree,
          selected: diff.pathFor(stashView.spec("worktree")),
          onselect: (path) => onopenstash("worktree", path),
        },
        {
          title: "Index",
          side: "index",
          files: parts.index,
          selected: diff.pathFor(stashView.spec("index")),
          onselect: (path) => onopenstash("index", path),
        },
        {
          title: "Untracked",
          files: parts.untracked,
          selected: diff.pathFor(stashView.spec("untracked")),
          onselect: (path) => onopenstash("untracked", path),
        },
      ]}
      empty="This stash is empty."
      selected={diff.path}
      onopen={onopenwindow}
      {onmarked}
    />
  {:else if shown.kind === "compare"}
    <FileList
      {activePanel}
      revisionFiles={fileTotal.value}
      context="compare"
      view={filesView.commit}
      onview={(next) => filesView.setCommit(next)}
      sections={[
        {
          title: `From ${compareView.from?.slice(0, 7)} to ${compareView.to?.slice(0, 7)}`,
          files: compareView.files,
          onselect: onopencompare,
        },
      ]}
      empty={emptyText(
        { settled: !compareView.loading, failed: compareView.error !== null },
        "Both commits have the same files.",
      )}
      selected={diff.path}
      onopen={onopenwindow}
      {onmarked}
    />
  {:else if shown.kind === "worktree"}
    <FileList
      {activePanel}
      revisionFiles={fileTotal.value}
      view={filesView.current}
      onview={onviewchange}
      {contents}
      split={fractions.filesSplit}
      onsplit={(delta) => layout.nudge("filesSplit", delta)}
      onsplitreset={() => layout.resetOne("filesSplit")}
      sections={separate
        ? [
            {
              title: "Unstaged",
              files: unstagedRows,
              selected: diff.pathFor({ kind: "workTreeVsIndex" }),
              onselect: onopenworktree,
              actions: [
                {
                  label: "Stage",
                  title: "Stage",
                  shortcut: shortcut("stage"),
                  run: stage,
                  blocked: rowBlocked("stage"),
                  skips: true,
                },
                { label: "+x", title: "Stage only the mode change", run: stagemode, blocked: rowBlocked("mode"), skips: true },
                { label: "Discard", title: "Discard changes", run: discard, blocked: rowBlocked("discard") },
                { label: "Ignore", title: "Add to .gitignore", run: ignore, blocked: rowBlocked("ignore") },
                {
                  label: "Delete",
                  title: "Move to the Recycle Bin (the Trash off Windows)",
                  run: remove,
                  blocked: rowBlocked("delete"),
                },
              ],
            },
            {
              title: "Staged",
              side: "index",
              files: worktree.staged,
              hideWhenEmpty: true,
              selected: diff.pathFor({ kind: "indexVsHead" }),
              onselect: onopenstaged,
              actions: [{ label: "Unstage", title: "Unstage", shortcut: shortcut("unstage"), run: unstage }],
            },
          ]
        : [
            {
              files: merged,
              selected: diff.pathFor({ kind: "workTreeVsIndex" }) ?? diff.pathFor({ kind: "indexVsHead" }),
              onselect: (path: string) =>
                mergedSide(path, unstagedRows) === "worktree" ? onopenworktree(path) : onopenstaged(path),
            },
          ]}
      empty={emptyText(
        { settled: worktree.loaded, failed: worktree.error !== null },
        "The working tree is clean.",
      )}
      selected={diff.path}
      onopen={(path, side) => onopenrow(path, separate ? (side === "index" ? "index" : "worktree") : mergedSide(path, unstagedRows))}
      unchanged={everyFile}
      onmask={(text) => {
        mask = text;
        onmask(text);
      }}
      onshown={(shown) => onshownstaged?.(shown && stagedShown(shown, separate, worktree.staged))}
      {onmarked}
      {oncontext}
    />
  {:else}
    <FileList
      {activePanel}
      revisionFiles={fileTotal.value}
      context="commit"
      view={filesView.commit}
      onview={(next) => filesView.setCommit(next)}
      sections={[{ files: commitList }]}
      empty={nothing}
      selected={diff.path}
      onselect={onopencommit}
      onopen={(path) => onopenpast(path)}
      {onmarked}
      {oncontext}
    />
  {/if}
</div>

<style>
  .files {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
</style>
