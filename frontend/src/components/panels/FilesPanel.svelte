<script lang="ts">
  import FileList from "$components/file-list/FileList.svelte";
  import type { FileView } from "$lib/file-view";
  import { commit } from "$stores/commit.svelte";
  import { diff } from "$stores/diff.svelte";
  import { filesView } from "$stores/files-view.svelte";
  import { layout } from "$stores/layout.svelte";
  import { stashView } from "$stores/stash-view.svelte";
  import { worktree } from "$stores/worktree.svelte";

  /** Two lists in one place: the working tree while nothing is selected, and the files a
      selected commit changed. They share the panel but almost nothing else. */
  interface Props {
    onWorkingTree: boolean;
    onviewchange: (next: FileView) => void;
    onopenworktree: (path: string) => void;
    onopenstaged: (path: string) => void;
    onopencommit: (path: string) => void;
    /** A stash file names which of the three parts it came from. */
    onopenstash: (part: "worktree" | "index" | "untracked", path: string) => void;
    onopenwindow: (path: string) => void;
    onmask: (mask: string) => void;
    onmarked: (paths: string[]) => void;
    stage: (paths: string[]) => void;
    stagemode: (paths: string[]) => void;
    unstage: (paths: string[]) => void;
    discard: (paths: string[]) => void;
    ignore: (paths: string[]) => void;
    remove: (paths: string[]) => void;
  }

  let {
    onWorkingTree,
    onviewchange,
    onopenworktree,
    onopenstaged,
    onopencommit,
    onopenstash,
    onopenwindow,
    onmask,
    onmarked,
    stage,
    stagemode,
    unstage,
    discard,
    ignore,
    remove,
  }: Props = $props();

  const fractions = $derived(layout.fractions);
</script>

<div class="files">
  {#if stashView.contents}
    {@const parts = stashView.contents}
    <FileList
      sections={[
        {
          title: "Working tree",
          files: parts.worktree,
          onselect: (path) => onopenstash("worktree", path),
        },
        { title: "Index", files: parts.index, onselect: (path) => onopenstash("index", path) },
        {
          title: "Untracked",
          files: parts.untracked,
          onselect: (path) => onopenstash("untracked", path),
        },
      ]}
      empty="This stash is empty."
      selected={diff.path}
      onopen={onopenwindow}
    />
  {:else if onWorkingTree}
    <FileList
      view={filesView.current}
      onview={onviewchange}
      split={fractions.filesSplit}
      onsplit={(delta) => layout.nudge("filesSplit", delta)}
      onsplitreset={() => layout.resetOne("filesSplit")}
      sections={[
        {
          title: "Unstaged",
          files: worktree.unstaged,
          onselect: onopenworktree,
          actions: [
            { label: "Stage", title: "Stage", run: stage },
            { label: "+x", title: "Stage only the mode change", run: stagemode },
            { label: "Discard", title: "Discard changes", run: discard },
            { label: "Ignore", title: "Add to .gitignore", run: ignore },
            { label: "Delete", title: "Delete from disk", run: remove },
          ],
        },
        {
          title: "Staged",
          files: worktree.staged,
          onselect: onopenstaged,
          actions: [{ label: "Unstage", title: "Unstage", run: unstage }],
        },
      ]}
      empty="The working tree is clean."
      selected={diff.path}
      onopen={onopenwindow}
      {onmask}
      {onmarked}
    />
  {:else}
    <FileList
      sections={[{ files: commit.files }]}
      empty="Select a commit to see the files it changed."
      selected={diff.path}
      onselect={onopencommit}
      onopen={onopenwindow}
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
