<script lang="ts">
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import type { RepoOverview } from "$lib/ipc";
  import type { ListedRepo } from "$lib/repo-list";
  import type { ModuleRow } from "$lib/module-tree";

  interface Props {
    filter: string;
    onselect: (entry: RepoOverview) => void;
    oncontext: (row: ListedRepo, x: number, y: number) => void;
    onreopen: (root: string) => void;
    onmarked: (roots: string[]) => void;
    ongroupcontext: (id: string, x: number, y: number) => void;
    onopenmodule: (row: ModuleRow) => void;

    onopenforeignmodule: (root: string, row: ModuleRow) => void;
    onmodulecontext: (row: ModuleRow, x: number, y: number, root?: string) => void;
  }

  let {
    filter,
    onselect,
    oncontext,
    onreopen,
    onmarked,
    ongroupcontext,
    onopenmodule,
    onopenforeignmodule,

    onmodulecontext,
  }: Props = $props();
</script>

<RepositoryList
  {filter}
  {onselect}
  {oncontext}
  {onreopen}
  {onmarked}
  {ongroupcontext}
  {onopenmodule}
  {onopenforeignmodule}
  {onmodulecontext}
/>
