<script lang="ts">
  import RefTree from "$components/branch-tree/RefTree.svelte";
  import type { RefNode, RefTreeInput } from "$lib/ref-nodes";
  import type { Branch } from "$lib/ipc";
  import { refs } from "$stores/refs.svelte";
  import { repository } from "$stores/repository.svelte";
  import { worktrees } from "$stores/worktrees.svelte";

  interface Props {
    input: RefTreeInput;
    /** Ticking a box changes which tips the graph walks, so the caller rebuilds it. */
    onvisible: () => void;
    onselect: (node: RefNode) => void;
    oncheckout: (branch: Branch) => void;
    onactivate: (node: RefNode) => void;
    oncontext: (node: RefNode, x: number, y: number) => void;
    ondrop: (source: string, target: Branch, x: number, y: number) => void;
  }

  let { input, onvisible, onselect, oncheckout, onactivate, oncontext, ondrop }: Props =
    $props();

  const sorted = $derived({
    ...input,
    sort: refs.sort,
    dates: refs.dates,
    worktrees: worktrees.entries,
  });

  // A re-read of the repository hands over new ref arrays, so a moved tip is re-dated; a
  // status refresh keeps them, and costs no read.
  const repo = $derived(repository.current?.repo);
  const branches = $derived(repository.current?.branches);
  const tags = $derived(repository.current?.tags);
  $effect(() => {
    void [branches, tags];
    if (repo === undefined || refs.sort.dates === "off") return;
    void refs.loadDates(repo);
  });
</script>

{#if repository.current}
  <RefTree
    input={sorted}
    visible={refs.visible}
    onvisible={(next) => {
      refs.set(next);
      onvisible();
    }}
    oncollapse={(id) => refs.collapse(id)}
    {onselect}
    {oncheckout}
    {onactivate}
    {oncontext}
    {ondrop}
  />
{/if}
