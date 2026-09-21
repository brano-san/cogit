<script lang="ts">
  import EmptyState from "$components/common/EmptyState.svelte";
  import CommitList from "$components/graph/CommitList.svelte";
  import PauseCheckBar from "$components/graph/PauseCheckBar.svelte";
  import type { HookRun, RebaseProgress } from "$lib/ipc";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** Non-null while a rebase is in flight; its steps become rows of the list below. */
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
    <PauseCheckBar {check} {oncheck} onrun={onruncheck} {verdict} running={checking} />
  {/if}
  <CommitList rebase={progress} {ondrop} {oncontext} {onref} />
{:else}
  <EmptyState
    title="No history to show"
    hint="Open a repository and its commits appear here."
  />
{/if}
