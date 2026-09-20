<script lang="ts">
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import SubmoduleList from "$components/repo-tree/SubmoduleList.svelte";
  import WorktreeList from "$components/repo-tree/WorktreeList.svelte";
  import { worktrees } from "$stores/worktrees.svelte";
  import type { WorktreeEntry } from "$lib/ipc";
  import type { RepoOverview, Submodule } from "$lib/ipc";
  import { submodules } from "$stores/submodules.svelte";

  interface Props {
    /** True only while the folder dialog's own flow runs, so the label does not flicker. */
    opening: boolean;
    onscan: () => void;
    onopen: () => void;
    onselect: (entry: RepoOverview) => void;
    onclose: (entry: RepoOverview) => void;
    oncontext: (entry: RepoOverview, x: number, y: number) => void;
    onmarked: (roots: string[]) => void;
    onopenworktree: (entry: WorktreeEntry) => void;
    onremoveworktree: (entry: WorktreeEntry) => void;
    onaddworktree: () => void;
    onpruneworktrees: () => void;
    onopenmodule: (module: Submodule) => void;
    onupdatemodule: (module: Submodule) => void;
  }

  let {
    opening,
    onscan,
    onopen,
    onselect,
    onclose,
    oncontext,
    onmarked,
    onopenworktree,
    onremoveworktree,
    onaddworktree,
    onpruneworktrees,
    onopenmodule,
    onupdatemodule,
  }: Props = $props();
</script>

<RepositoryList {opening} {onscan} {onopen} {onselect} {onclose} {oncontext} {onmarked} />
<WorktreeList
  entries={worktrees.entries}
  onopen={onopenworktree}
  onremove={onremoveworktree}
  onadd={onaddworktree}
  onprune={onpruneworktrees}
/>
<SubmoduleList
  modules={submodules.entries}
  onopen={(module) => onopenmodule(module)}
  onupdate={(module) => onupdatemodule(module)}
/>
