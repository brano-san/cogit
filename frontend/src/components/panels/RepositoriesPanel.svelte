<script lang="ts">
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
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
