<script lang="ts">
  import Button from "$components/common/Button.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import EmptyState from "$components/common/EmptyState.svelte";
  import FileStateIcon from "$components/common/FileStateIcon.svelte";
  import Select from "$components/common/Select.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import VirtualList from "$components/common/VirtualList.svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import ImageDiff from "$components/diff/ImageDiff.svelte";
  import Notifications from "$components/layout/Notifications.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import { untrack } from "svelte";
  import { installChildWindow } from "$lib/child-window";
  import { fileState } from "$lib/file-state";
  import { shortOid } from "$lib/format";
  import { onRepoChanged, repoRefs, type FileEntry } from "$lib/ipc";
  import { REVIEW_SHOWN_COMMITS, baseOptions, baseCaption, reloadsOn } from "$lib/review-view";
  import { parseReview, reviewTitle } from "$lib/review-params";
  import { followSettings } from "$lib/settings-sync";
  import { diff } from "$stores/diff.svelte";
  import { ReviewStore } from "$stores/review.svelte";
  import { settings } from "$stores/settings.svelte";
  import { uiMetricsStore } from "$stores/ui-metrics.svelte";

  const request = parseReview(window.location.search);
  const review = new ReviewStore((repo, spec, path) => void diff.load(repo, spec, path));

  $effect(() => installChildWindow(window));
  $effect(() => followSettings(() => void settings.reload()));

  let branches = $state.raw<string[]>([]);
  let leftFraction = $state(0.36);
  let commitsFraction = $state(0.34);

  $effect(() =>
    untrack(() => {
      if (!request) return;
      void (async () => {
        await settings.ready();
        diff.whitespace = settings.current.ignoreWhitespace;
        await review.open(request.repo, request.branch, request.base);
      })();
      void repoRefs(request.repo)
        .then((refs) => (branches = refs.branches.map((branch) => branch.name)))
        .catch(() => {});
    }),
  );

  $effect(() => {
    if (!request) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const pending = onRepoChanged((change) => {
      if (change.repo !== request.repo || !reloadsOn(change.kind, review.worktree)) return;
      clearTimeout(timer);
      timer = setTimeout(() => void review.reload(), 300);
    });
    return () => {
      clearTimeout(timer);
      void pending.then((stop) => stop()).catch(() => {});
    };
  });

  $effect(() => {
    const title = request ? reviewTitle(request.branch, request.repoName) : "Cogit — Review Branch";
    document.title = title;
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().setTitle(title)).catch(() => {});
  });

  const options = $derived(baseOptions(branches, request?.branch ?? "", review.base));
  const summary = $derived(review.summary);
  const totals = $derived(review.totals);
  const commitRows = $derived(summary ? [null, ...summary.commits] : []);

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "F5") {
      event.preventDefault();
      void review.reload();
    }
  }

  function counts(file: FileEntry): string {
    const found = review.countsOf(file);
    if (!found) return "";
    if (found.added === null || found.removed === null) return "bin";
    return `+${found.added} −${found.removed}`;
  }

  function date(timestamp: number, offset: number): string {
    return settings.formatDate(timestamp, offset);
  }
</script>

<svelte:window {onkeydown} />

<TooltipLayer />
<Notifications onopenurl={() => {}} onshowoutput={() => {}} onaction={() => {}} />

<div class="window">
  {#if !request}
    <EmptyState title="This window needs a branch to review. Open it from a branch's context menu rather than by hand." />
  {:else}
    <header>
      <span class="title truncate">Review <b>{request.branch}</b></span>
      <span class="label">against</span>
      <span class="base">
        <Select
          value={review.base ?? ""}
          options={options}
          label="Base branch"
          onchange={(value) => value && void review.setBase(value)}
        />
      </span>
      {#if summary}
        <span class="facts truncate" title={baseCaption(summary)}>
          merge base <span class="mono">{shortOid(summary.mergeBase)}</span> ·
          {summary.ahead} commit{summary.ahead === 1 ? "" : "s"} ahead, {summary.behind} behind
        </span>
      {:else}
        <span class="facts"></span>
      {/if}
      <Checkbox
        label="Include working tree"
        checked={review.worktree}
        disabled={!review.canIncludeWorktree || review.commit !== null}
        title={review.canIncludeWorktree ? "Add the uncommitted changes of the checked-out branch" : "Only the checked-out branch has a working tree"}
        onchange={(on) => void review.setWorktree(on)}
      />
      <Button onclick={() => void review.reload()} title="Refresh (F5)">Refresh</Button>
    </header>
    {#if review.note}
      <p class="note" role="status">{review.note}</p>
    {/if}

    {#if review.error}
      <EmptyState tone="error" title={review.error.message} hint="Choose another base above." />
    {:else if review.problem}
      <EmptyState title={review.problem} />
    {:else if !summary}
      <EmptyState title="Loading…" />
    {:else}
      <div class="body">
        <div class="left" style:flex-basis="{leftFraction * 100}%">
          <div class="commits" style:flex-basis="{commitsFraction * 100}%">
            <div class="heading">
              <span>Commits</span>
              <span class="muted tabular">
                {summary.ahead > summary.commits.length ? `${summary.commits.length} of ${summary.ahead}` : summary.ahead}
              </span>
            </div>
            <VirtualList items={commitRows} label="Commits of the branch">
              {#snippet row(commit, index)}
                {@const picked = commit === null ? review.commit === null : review.commit === commit.oid}
                <button
                  type="button"
                  class="row commit"
                  class:selected={picked}
                  style:top="{index * uiMetricsStore.listRow}px"
                  onclick={() => void review.selectCommit(commit?.oid ?? null)}
                  title={commit ? `${commit.summary}\n${commit.author.name}, ${date(commit.author.timestamp, commit.author.tzOffsetMinutes)}` : "Every change of the branch"}
                >
                  {#if commit}
                    <span class="mono muted">{shortOid(commit.oid)}</span>
                    <span class="truncate grow">{commit.summary}</span>
                    <span class="muted truncate author">{commit.author.name}</span>
                  {:else}
                    <span class="grow">All changes</span>
                  {/if}
                </button>
              {/snippet}
            </VirtualList>
            {#if summary.ahead > REVIEW_SHOWN_COMMITS}
              <p class="muted small">The first {REVIEW_SHOWN_COMMITS} commits are listed.</p>
            {/if}
          </div>
          <Splitter
            direction="vertical"
            value={commitsFraction}
            label="Resize Commits"
            onchange={(delta) => (commitsFraction = Math.min(0.8, Math.max(0.1, commitsFraction + delta)))}
            onreset={() => (commitsFraction = 0.34)}
          />
          <div class="files">
            <div class="heading">
              <span>{review.commit ? `Files of ${shortOid(review.commit)}` : "Files"}</span>
              <span class="muted tabular">{review.shown.length}</span>
              <span class="grow"></span>
              {#if totals.counted > 0}
                <span class="tabular added">+{totals.added}</span>
                <span class="tabular removed">−{totals.removed}</span>
              {/if}
            </div>
            {#if review.merged && review.commit === null}
              <EmptyState
                title="Nothing to review: {request.branch} has no commits that {review.base} lacks."
                hint="The branch is already merged into its base."
              />
            {:else if review.shown.length === 0 && !review.listing}
              <EmptyState title="No changed files" />
            {:else}
              <VirtualList items={review.shown} label="Changed files">
                {#snippet row(file, index)}
                  {@const state = fileState(file, "commit")}
                  <button
                    type="button"
                    class="row file"
                    class:selected={review.selected === file.path}
                    style:top="{index * uiMetricsStore.listRow}px"
                    title={file.oldPath ? `${state.tooltip}: ${file.oldPath} → ${file.path}` : `${state.tooltip}: ${file.path}`}
                    onclick={() => review.select(file.path)}
                  >
                    <FileStateIcon base={file.mode === "submodule" ? "submodule" : "file"} state={state.icon} />
                    {#if file.oldPath}<span class="muted truncate old">{file.oldPath} →</span>{/if}
                    <span class="truncate grow">{file.path}</span>
                    <span class="tabular muted counts">{counts(file)}</span>
                  </button>
                {/snippet}
              </VirtualList>
            {/if}
          </div>
        </div>
        <Splitter
          direction="horizontal"
          value={leftFraction}
          label="Resize Files"
          onchange={(delta) => (leftFraction = Math.min(0.7, Math.max(0.15, leftFraction + delta)))}
          onreset={() => (leftFraction = 0.36)}
        />
        <div class="diff">
          {#if review.selected === null}
            <EmptyState title="Select a file to see its changes" />
          {:else if diff.error}
            <EmptyState tone="error" title={diff.error.message} />
          {:else if diff.diff?.kind === "submodule"}
            <EmptyState title="{review.selected} is a submodule: open it in the main window to review it." />
          {:else if diff.diff?.kind === "image"}
            <ImageDiff
              before={diff.images[0]}
              after={diff.images[1]}
              oldSize={diff.diff.oldSize}
              newSize={diff.diff.newSize}
              mime={diff.diff.mime}
            />
          {:else if diff.diff && diff.shownPath}
            <DiffView
              diff={diff.diff}
              path={diff.shownPath}
              stageable={false}
              whitespace={diff.whitespace}
              onwhitespace={(mode) => void diff.setWhitespace(request.repo, mode)}
            />
          {:else}
            <EmptyState title="Loading…" />
          {/if}
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-base);
    color: var(--text-primary);
    font-family: var(--font-ui);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-height: var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .title {
    flex: 0 1 auto;
    max-width: 30%;
  }

  .label,
  .muted {
    color: var(--text-secondary);
  }

  .base {
    display: flex;
    flex: 0 1 220px;
    min-width: 120px;
  }

  .facts {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
  }

  .note {
    margin: 0;
    padding: var(--sp-2) var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .body {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .left {
    display: flex;
    flex: 0 0 auto;
    flex-direction: column;
    min-width: 200px;
    border-right: 1px solid var(--divider);
  }

  .commits {
    display: flex;
    flex: 0 0 auto;
    flex-direction: column;
    min-height: 60px;
  }

  .files {
    display: flex;
    flex: 1 1 0;
    flex-direction: column;
    min-height: 60px;
  }

  .heading {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    height: var(--h-row);
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
    font-weight: 600;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-5);
    background: none;
    border: none;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    white-space: nowrap;
    cursor: default;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--selected-bar);
  }

  .grow {
    flex: 1 1 auto;
    min-width: 0;
  }

  .author {
    flex: 0 1 auto;
    max-width: 30%;
  }

  .old {
    flex: 0 1 auto;
    max-width: 45%;
  }

  .counts {
    flex: 0 0 auto;
  }

  .added {
    color: var(--status-success);
  }

  .removed {
    color: var(--status-danger);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .small {
    margin: 0;
    padding: var(--sp-2) var(--sp-5);
    font-size: var(--fs-dense);
  }

  .diff {
    display: flex;
    flex: 1 1 0;
    flex-direction: column;
    min-width: 0;
  }
</style>
