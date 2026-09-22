<script lang="ts">
  import StartScreen from "$components/layout/StartScreen.svelte";
  import CommitList from "$components/graph/CommitList.svelte";
  import PauseCheckBar from "$components/graph/PauseCheckBar.svelte";
  import type { HookRun, RebaseProgress } from "$lib/ipc";
  import { panelView } from "$lib/repo-phase";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** Non-null while a rebase is in flight; its steps become rows of the list below. */
    progress: RebaseProgress | null;
    /** Shown in place of the history while nothing is open (M3 T3.6). */
    recent: readonly string[];
    onopenrecent: (root: string) => void;
    onforgetrecent: (root: string) => void;
    onopen: () => void;
    onscan: () => void;
    check: string;
    oncheck: (command: string) => void;
    onruncheck: () => void;
    verdict: HookRun | null;
    checking: boolean;
    ondrop: (source: string, target: string) => void;
    oncontext: (oid: string, x: number, y: number) => void;
    onref: (text: string) => void;
  }

  let {
    progress,
    recent,
    onopenrecent,
    onforgetrecent,
    onopen,
    onscan,
    check,
    oncheck,
    onruncheck,
    verdict,
    checking,
    ondrop,
    oncontext,
    onref,
  }: Props = $props();

  const view = $derived(panelView(repository.phase));
</script>

{#if view === "opening"}
  <p class="waiting">Opening repository…</p>
{:else if view === "content"}
  {#if progress}
    <PauseCheckBar {check} {oncheck} onrun={onruncheck} {verdict} running={checking} />
  {/if}
  <CommitList rebase={progress} {ondrop} {oncontext} {onref} />
{:else}
  <StartScreen
    {recent}
    {onopen}
    {onscan}
    onpick={onopenrecent}
    onforget={onforgetrecent}
  />
{/if}

<style>
  .waiting {
    margin: 0;
    padding: var(--sp-5);
    color: var(--text-secondary);
  }
</style>
