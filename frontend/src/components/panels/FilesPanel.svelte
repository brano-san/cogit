<script lang="ts">
  import FileList from "$components/file-list/FileList.svelte";
  import { ContentSearch } from "$lib/content-search.svelte";
  import { rowActionBlocked, type RowAction } from "$lib/file-menu";
  import { withUnchanged } from "$lib/file-switches";
  import { emptyText, filesPanelList } from "$lib/files-panel";
  import type { FileView } from "$lib/file-view";
  import type { FileEntry } from "$lib/ipc";
  import { idleMessage } from "$lib/repo-phase";
  import { commit } from "$stores/commit.svelte";
  import { commitTree } from "$stores/commit-tree.svelte";
  import { compareView } from "$stores/compare-view.svelte";
  import { diff } from "$stores/diff.svelte";
  import { errors } from "$stores/errors.svelte";
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
    onmask: (mask: string) => void;
    /** The staged files the working-tree list shows once filtered. */
    onshownstaged?: (paths: string[]) => void;
    onmarked: (paths: string[]) => void;
    oncontext: (path: string, event: MouseEvent, section?: string) => void;
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

  /** A row button off where the Files menu has its item off (#40). */
  const rowBlocked = (action: RowAction) => (file: FileEntry) => rowActionBlocked(action, file);

  /** Three different nothings, and the panel used to say the same thing for all of them. */
  const nothing = $derived(
    view !== "content"
      ? (idleMessage(view) ?? "")
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
      view={filesView.current}
      onview={onviewchange}
      {contents}
      split={fractions.filesSplit}
      onsplit={(delta) => layout.nudge("filesSplit", delta)}
      onsplitreset={() => layout.resetOne("filesSplit")}
      sections={[
        {
          title: "Unstaged",
          files: worktree.unstaged,
          selected: diff.pathFor({ kind: "workTreeVsIndex" }),
          onselect: onopenworktree,
          actions: [
            { label: "Stage", title: "Stage", run: stage },
            { label: "+x", title: "Stage only the mode change", run: stagemode, blocked: rowBlocked("mode") },
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
          files: worktree.staged,
          hideWhenEmpty: true,
          selected: diff.pathFor({ kind: "indexVsHead" }),
          onselect: onopenstaged,
          actions: [{ label: "Unstage", title: "Unstage", run: unstage }],
        },
      ]}
      empty={emptyText(
        { settled: worktree.loaded, failed: worktree.error !== null },
        "The working tree is clean.",
      )}
      selected={diff.path}
      onopen={onopenwindow}
      {onmask}
      onshown={(shown) => onshownstaged?.(shown[1] ?? [])}
      {onmarked}
      {oncontext}
    />
  {:else}
    <FileList
      {activePanel}
      context="commit"
      view={filesView.commit}
      onview={(next) => filesView.setCommit(next)}
      sections={[{ files: commitList }]}
      empty={nothing}
      selected={diff.path}
      onselect={onopencommit}
      onopen={onopenwindow}
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
