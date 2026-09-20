<script lang="ts">
  import CommitList from "$components/graph/CommitList.svelte";
  import RebaseProgressView from "$components/graph/RebaseProgressView.svelte";
  import type { RebaseProgress } from "$lib/ipc";
  import { repository } from "$stores/repository.svelte";
  import { worktree } from "$stores/worktree.svelte";

  interface Props {
    /** Non-null while a rebase is in flight; the banner sits above the history. */
    progress: RebaseProgress | null;
    ondrop: (source: string, target: string) => void;
    oncontext: (oid: string, x: number, y: number) => void;
    onref: (text: string) => void;
  }

  let { progress, ondrop, oncontext, onref }: Props = $props();
</script>

{#if repository.current}
  {#if progress}
    <RebaseProgressView {progress} changes={worktree.total} staged={worktree.staged.length} />
  {/if}
  <CommitList {ondrop} {oncontext} {onref} />
{:else}
  <p class="note">Open a repository to see its history.</p>
{/if}

<style>
  .note {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }
</style>
