<script lang="ts">
  import CommitList from "$components/graph/CommitList.svelte";
  import RebaseProgressView from "$components/graph/RebaseProgressView.svelte";
  import type { HookRun, RebaseProgress } from "$lib/ipc";
  import { repository } from "$stores/repository.svelte";
  import { worktree } from "$stores/worktree.svelte";

  interface Props {
    /** Non-null while a rebase is in flight; the banner sits above the history. */
    progress: RebaseProgress | null;
    check: string;
    oncheck: (command: string) => void;
    onruncheck: () => void;
    verdict: HookRun | null;
    checking: boolean;
    ondrop: (source: string, target: string) => void;
    oncontext: (oid: string, x: number, y: number) => void;
    onref: (text: string) => void;
  }

  let { progress, check, oncheck, onruncheck, verdict, checking, ondrop, oncontext, onref }: Props =
    $props();
</script>

{#if repository.current}
  {#if progress}
    <RebaseProgressView
      {progress}
      changes={worktree.total}
      staged={worktree.staged.length}
      {check}
      {oncheck}
      onrun={onruncheck}
      {verdict}
      running={checking}
    />
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
