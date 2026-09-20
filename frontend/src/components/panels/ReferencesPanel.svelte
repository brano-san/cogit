<script lang="ts">
  import RefTree from "$components/branch-tree/RefTree.svelte";
  import type { RefNode, RefTreeInput } from "$lib/ref-nodes";
  import type { Branch } from "$lib/ipc";
  import { refs } from "$stores/refs.svelte";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    input: RefTreeInput;
    /** Ticking a box changes which tips the graph walks, so the caller rebuilds it. */
    onvisible: () => void;
    onselect: (node: RefNode) => void;
    oncheckout: (branch: Branch) => void;
    onactivate: (node: RefNode) => void;
    oncontext: (node: RefNode, x: number, y: number) => void;
    ondrop: (source: string, target: Branch) => void;
  }

  let { input, onvisible, onselect, oncheckout, onactivate, oncontext, ondrop }: Props =
    $props();
</script>

{#if repository.current}
  <RefTree
    {input}
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
