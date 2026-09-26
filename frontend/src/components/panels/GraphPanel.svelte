<script lang="ts">
  import StartScreen from "$components/layout/StartScreen.svelte";
  import CommitList from "$components/graph/CommitList.svelte";
  import GraphFilterFields from "$components/graph/GraphFilterFields.svelte";
  import PauseCheckBar from "$components/graph/PauseCheckBar.svelte";
  import StateBanner from "$components/layout/StateBanner.svelte";
  import type { Banner, BannerAction } from "$lib/repo-state";
  import type { HookRun, RebaseProgress } from "$lib/ipc";
  import { panelView } from "$lib/repo-phase";
  import { repository } from "$stores/repository.svelte";
  import { settings } from "$stores/settings.svelte";
  import { graphFilter } from "$stores/graph-filter.svelte";

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
    ondrop: (source: string, target: string, x: number, y: number) => void;
    oncontext: (oid: string, x: number, y: number) => void;
    /** A merge, rebase or detached HEAD is said above the history, as SmartGit does (#22). */
    banner: Banner | null;
    busy: boolean;
    onbanneraction: (action: BannerAction) => void;
    onworktreecontext?: (x: number, y: number) => void;
    onrefcontext?: (label: import("$lib/format").RefLabel, oid: string, x: number, y: number) => void;
    /** A filter left the list empty; its button clears the filter. */
    onclearfilter?: () => void;
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
    banner,
    busy,
    onbanneraction,
    onworktreecontext,
    onrefcontext,
    onclearfilter,
  }: Props = $props();

  const view = $derived(panelView(repository.phase));
</script>

{#if view === "opening"}
  <!-- Blank on purpose: the footer is the one place an open in progress is reported (#4). -->
{:else if view === "content"}
  {#if banner}
    <StateBanner {banner} {busy} onaction={onbanneraction} />
  {/if}
  {#if graphFilter.showsFields}
    <GraphFilterFields />
  {/if}
  {#if progress}
    <PauseCheckBar {check} {oncheck} onrun={onruncheck} {verdict} running={checking} />
  {/if}
  <CommitList
    rebase={progress}
    {ondrop}
    {oncontext}
    {onworktreecontext}
    {onrefcontext}
    {onclearfilter}
    columns={settings.current.graphColumns}
    timeFormat={settings.current.graphTimeFormat}
    density={settings.current.graphDensity}
    stripes={settings.current.graphStripes}
    longLinkRows={settings.current.graphLongLinkRows}
    highlightChecked={settings.current.graphHighlightChecked}
    firstParent={settings.current.graphFirstParent}
    branchOfCommit={settings.current.graphBranchOfCommit}
    ancestry={settings.current.graphAncestry}
    collapseMerged={settings.current.graphCollapseMerged}
  />
{:else}
  <StartScreen
    {recent}
    {onopen}
    {onscan}
    onpick={onopenrecent}
    onforget={onforgetrecent}
  />
{/if}
