<script lang="ts">
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import SubmoduleList from "$components/repo-tree/SubmoduleList.svelte";
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
    onopenmodule,
    onupdatemodule,
  }: Props = $props();
</script>

<RepositoryList {opening} {onscan} {onopen} {onselect} {onclose} {oncontext} />
<SubmoduleList
  modules={submodules.entries}
  onopen={(module) => onopenmodule(module)}
  onupdate={(module) => onupdatemodule(module)}
/>
