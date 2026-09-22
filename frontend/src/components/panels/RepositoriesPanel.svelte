<script lang="ts">
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import WorktreeList from "$components/repo-tree/WorktreeList.svelte";
  import { worktrees } from "$stores/worktrees.svelte";
  import type { WorktreeEntry } from "$lib/ipc";
  import type { RepoOverview } from "$lib/ipc";
  import type { ModuleRow } from "$lib/module-tree";

  interface Props {
    /** True only while the folder dialog's own flow runs, so the label does not flicker. */
    opening: boolean;
    onscan: () => void;
    onopen: () => void;
    onselect: (entry: RepoOverview) => void;
    onclose: (entry: RepoOverview) => void;
    oncontext: (entry: RepoOverview, x: number, y: number) => void;
    onmarked: (roots: string[]) => void;
    ongroupcontext: (id: string, x: number, y: number) => void;
    onaddgroup: () => void;
    onopenworktree: (entry: WorktreeEntry) => void;
    onremoveworktree: (entry: WorktreeEntry) => void;
    onaddworktree: () => void;
    onpruneworktrees: () => void;
    onopenmodule: (row: ModuleRow) => void;

    onmodulecontext: (row: ModuleRow, x: number, y: number) => void;
  }

  let {
    opening,
    onscan,
    onopen,
    onselect,
    onclose,
    oncontext,
    onmarked,
    ongroupcontext,
    onaddgroup,
    onopenworktree,
    onremoveworktree,
    onaddworktree,
    onpruneworktrees,
    onopenmodule,

    onmodulecontext,
  }: Props = $props();
</script>

<RepositoryList
  {opening}
  {onscan}
  {onopen}
  {onselect}
  {onclose}
  {oncontext}
  {onmarked}
  {ongroupcontext}
  {onaddgroup}
  {onopenmodule}
  {onmodulecontext}
/>
<WorktreeList
  entries={worktrees.entries}
  onopen={onopenworktree}
  onremove={onremoveworktree}
  onadd={onaddworktree}
  onprune={onpruneworktrees}
/>
