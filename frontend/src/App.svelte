<script lang="ts">
  import { ask, open as openFolderDialog } from "@tauri-apps/plugin-dialog";

  import BranchList from "$components/branch-tree/BranchList.svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import CommitBox from "$components/file-list/CommitBox.svelte";
  import FileList from "$components/file-list/FileList.svelte";
  import CommitList from "$components/graph/CommitList.svelte";
  import GraphFilter from "$components/graph/GraphFilter.svelte";
  import Panel from "$components/layout/Panel.svelte";
  import GitErrorDialog from "$components/layout/GitErrorDialog.svelte";
  import OutputPanel from "$components/layout/OutputPanel.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import StatusBar from "$components/layout/StatusBar.svelte";
  import Toolbar from "$components/layout/Toolbar.svelte";
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import { formatCommitDate, shortOid } from "$lib/format";
  import {
    checkout,
    deleteBranch,
    getAppInfo,
    onRepoChanged,
    type AppInfo,
    type Branch,
  } from "$lib/ipc";
  import { commit } from "$stores/commit.svelte";
  import { worktree } from "$stores/worktree.svelte";
  import { diff } from "$stores/diff.svelte";
  import { errors } from "$stores/errors.svelte";
  import { output } from "$stores/output.svelte";
  import { safety } from "$stores/safety.svelte";
  import { graph } from "$stores/graph.svelte";
  import { layout } from "$stores/layout.svelte";
  import { repository } from "$stores/repository.svelte";

  let info = $state<AppInfo | null>(null);

  $effect(() => {
    getAppInfo().then((result) => {
      info = result;
    });
  });

  const fractions = $derived(layout.fractions);
  const repo = $derived(repository.current);
  const details = $derived(commit.details);

  const onWorkingTree = $derived(repo !== undefined && repo !== null && commit.oid === null);

  $effect(() => {
    void commit.oid;
    diff.clear();
  });

  $effect(() => {
    const id = repository.current?.repo;
    if (id && commit.oid === null) void worktree.load(id);
  });

  // Every failure that carries raw Git output goes to the dialog; INV-05 says the user
  // sees exactly what Git said, not a summary of it.
  $effect(() => errors.report(worktree.error));
  $effect(() => errors.report(repository.error));
  $effect(() => errors.report(commit.error));
  $effect(() => errors.report(diff.error));
  $effect(() => errors.report(graph.error));

  // Every Git command lands in the journal; the panel reads it back when it is open.
  $effect(() => {
    void worktree.loading;
    void repository.busy;
    void output.refreshProblems();
    void safety.refresh();
    if (output.open) void output.refresh();
  });

  function onkeydown(event: KeyboardEvent) {
    if (event.ctrlKey && event.shiftKey && event.key === "&") {
      event.preventDefault();
      output.toggle();
    }
  }

  // The watcher is the only way Cogit learns about work done in a terminal alongside it.
  $effect(() => {
    const unlisten = onRepoChanged((change) => {
      const id = repository.current?.repo;
      if (!id || id.valueOf() !== change.repo.valueOf()) return;
      void repository.refresh();
      if (commit.oid === null) void worktree.load(id);
      if (change.kind === "head" || change.kind === "refs") void graph.load(id, graph.query);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  });

  function filterGraph(query: import("$lib/ipc").CommitQuery) {
    const id = repository.current?.repo;
    if (!id) return;
    commit.clear();
    diff.clear();
    void graph.load(id, query);
  }

  function stage(paths: string[]) {
    const id = repository.current?.repo;
    if (id) void worktree.stage(id, paths);
  }

  function unstage(paths: string[]) {
    const id = repository.current?.repo;
    if (id) void worktree.unstage(id, paths);
  }

  async function discard(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    const what = paths.length === 1 ? paths[0] : `${paths.length} files`;
    const confirmed = await ask(`Discard changes in ${what}? This cannot be undone.`, {
      title: "Discard changes",
      kind: "warning",
    });
    if (confirmed) void worktree.discard(id, paths);
  }

  async function commitStaged(message: string, amend: boolean, noVerify: boolean) {
    const id = repository.current?.repo;
    if (!id) return;
    await worktree.commit(id, message, amend, noVerify);
    if (worktree.error) return;
    await repository.refresh();
    void graph.load(id, graph.query);
  }

  async function afterRefChange() {
    const id = repository.current?.repo;
    if (!id) return;
    commit.clear();
    diff.clear();
    await repository.refresh();
    await worktree.load(id);
    void graph.load(id, graph.query);
  }

  async function switchTo(branch: Branch) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await checkout(id, { kind: "branch", name: branch.name });
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function removeBranch(branch: Branch) {
    const id = repository.current?.repo;
    if (!id) return;
    const confirmed = await ask(`Delete branch ${branch.name}?`, {
      title: "Delete branch",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await deleteBranch(id, branch.name, false);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function undo() {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await safety.undo(id);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  function openDiff(path: string) {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (id && oid) void diff.load(id, { kind: "commitVsParent", oid }, path);
  }

  function openStagedDiff(path: string) {
    const id = repository.current?.repo;
    if (id) void diff.load(id, { kind: "indexVsHead" }, path);
  }

  function openWorktreeDiff(path: string) {
    const id = repository.current?.repo;
    if (id) void diff.load(id, { kind: "workTreeVsIndex" }, path);
  }

  async function pickRepository() {
    const picked = await openFolderDialog({ directory: true, title: "Open Repository" });
    if (typeof picked !== "string") return;
    commit.clear();
    diff.clear();
    worktree.clear();
    await repository.open(picked);
    const opened = repository.current;
    if (opened) {
      void graph.load(opened.repo);
    } else {
      graph.clear();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="app">
  <Toolbar
    busy={repository.busy ? "Opening repository…" : undefined}
    undoable={safety.last?.description}
    onundo={undo}
  />

  <div class="workspace">
    <div class="left-column" style:flex="0 0 {fractions.leftColumn * 100}%">
      <div class="pane" style:flex="0 0 {fractions.repositories * 100}%">
        <Panel title="Repositories" count={repo ? 1 : 0}>
          <RepositoryList onopen={pickRepository} />
        </Panel>
      </div>
      <Splitter
        direction="horizontal"
        value={fractions.repositories}
        label="Resize repositories panel"
        onchange={(d) => layout.nudge("repositories", d)}
        onreset={() => layout.resetOne("repositories")}
      />
      <div class="pane grow">
        <Panel
          title="References"
          count={repo?.branches.length}
          empty={repo ? undefined : "Open a repository to see its branches."}
        >
          {#if repo}
            {#if repo.branches.length === 0}
              <p class="note">No branches yet — the first commit creates one.</p>
            {:else}
              <BranchList
                title="Local Branches"
                branches={repository.localBranches}
                oncheckout={switchTo}
                ondelete={removeBranch}
              />
              <BranchList
                title="Remote"
                branches={repository.remoteBranches}
                oncheckout={switchTo}
              />
            {/if}
          {/if}
        </Panel>
      </div>
    </div>

    <Splitter
      direction="vertical"
      value={fractions.leftColumn}
      label="Resize left column"
      onchange={(d) => layout.nudge("leftColumn", d)}
      onreset={() => layout.resetOne("leftColumn")}
    />

    <div class="right-area">
      <div class="top-row" style:flex="0 0 {fractions.topRow * 100}%">
        <div class="pane" style:flex="0 0 {fractions.graph * 100}%">
          <Panel title="Graph &amp; History" count={graph.rows.length}>
            {#snippet actions()}
              {#if repo}
                <GraphFilter onchange={filterGraph} matches={graph.rows.length} />
              {/if}
            {/snippet}
            {#if repo}
              <CommitList />
            {:else}
              <p class="note">Open a repository to see its history.</p>
            {/if}
          </Panel>
        </div>
        <Splitter
          direction="vertical"
          value={fractions.graph}
          label="Resize graph panel"
          onchange={(d) => layout.nudge("graph", d)}
          onreset={() => layout.resetOne("graph")}
        />
        <div class="pane grow">
          <Panel title="Files" count={onWorkingTree ? worktree.total : commit.files.length}>
            <div class="files">
            {#if onWorkingTree}
              <FileList
                sections={[
                  {
                    title: "Staged",
                    files: worktree.staged,
                    onselect: openStagedDiff,
                    actions: [{ label: "Unstage", title: "Unstage", run: unstage }],
                  },
                  {
                    title: "Unstaged",
                    files: worktree.unstaged,
                    onselect: openWorktreeDiff,
                    actions: [
                      { label: "Stage", title: "Stage", run: stage },
                      { label: "Discard", title: "Discard changes", run: discard },
                    ],
                  },
                ]}
                empty="The working tree is clean."
                selected={diff.path}
              />
              <CommitBox
                stagedCount={worktree.staged.length}
                busy={worktree.loading}
                draftKey={`cogit:draft:${repo?.root ?? ""}`}
                oncommit={commitStaged}
              />
            {:else}
              <FileList
                sections={[{ files: commit.files }]}
                empty="Select a commit to see the files it changed."
                selected={diff.path}
                onselect={openDiff}
              />
            {/if}
            </div>
          </Panel>
        </div>
      </div>

      <Splitter
        direction="horizontal"
        value={fractions.topRow}
        label="Resize diff panel"
        onchange={(d) => layout.nudge("topRow", d)}
        onreset={() => layout.resetOne("topRow")}
      />

      <div class="pane grow">
        <Panel title="Diff">
          {#if diff.error}
            <p class="error detail">{diff.error.message}</p>
          {:else if diff.diff && diff.path}
            <DiffView diff={diff.diff} path={diff.path} />
          {:else}
          <div class="detail">
            {#if repository.error}
              <p class="error">{repository.error.message}</p>
              {#if repository.error.isCommandFailure && repository.error.detail.kind === "command"}
                <pre class="raw">{repository.error.detail.data.stderr}</pre>
              {/if}
            {:else if commit.error}
              <p class="error">{commit.error.message}</p>
            {:else if details}
              <p class="subject">{details.summary}</p>
              {#if details.body}<pre class="body">{details.body}</pre>{/if}
              <dl>
                <dt>Commit</dt>
                <dd class="mono">{details.oid}</dd>
                <dt>Author</dt>
                <dd>
                  {details.author.name} &lt;{details.author.email}&gt; ·
                  {formatCommitDate(details.author.timestamp, details.author.tzOffsetMinutes)}
                </dd>
                <dt>Parents</dt>
                <dd class="mono tabular">
                  {details.parents.length === 0
                    ? "none (root commit)"
                    : details.parents.map(shortOid).join(", ")}
                </dd>
              </dl>
              <p class="muted">Select a file to see the diff.</p>
            {:else if repo}
              <dl>
                <dt>Repository</dt>
                <dd class="mono">{repo.root}</dd>
                <dt>HEAD</dt>
                <dd class="mono">{repository.headLabel}</dd>
                <dt>Branches</dt>
                <dd class="mono tabular">
                  {repository.localBranches.length} local, {repository.remoteBranches.length} remote
                </dd>
              </dl>
              <p class="muted">Select a commit to see what it changed.</p>
            {:else}
              <p class="muted">Open a repository to begin.</p>
            {/if}
          </div>
          {/if}
        </Panel>
      </div>
    </div>
  </div>

  {#if output.open}
    <OutputPanel />
  {/if}

  {#if errors.current}
    <GitErrorDialog error={errors.current} ondismiss={() => errors.dismiss()} />
  {/if}

  <StatusBar
    repository={repo?.name ?? "No repository"}
    branch={repo ? repository.headLabel : undefined}
    summary={repo ? `${graph.rows.length} commits · ${repo.branches.length} refs` : "Milestone C"}
    version={info?.version}
    status={repository.error ? "Error" : repository.busy ? "Working…" : "Ready"}
    problems={output.problems}
    onproblems={() => output.toggle()}
  />
</div>

<style>
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-base);
  }

  .workspace {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .left-column,
  .right-area {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .right-area {
    flex: 1 1 0;
  }

  .top-row {
    display: flex;
    min-height: 0;
    min-width: 0;
  }

  .pane {
    display: flex;
    min-width: 0;
    min-height: 0;
  }

  .pane > :global(.panel) {
    flex: 1 1 auto;
  }

  .grow {
    flex: 1 1 0;
  }

  .files {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .note {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .detail {
    padding: var(--sp-6);
    font-size: var(--fs-dense);
    user-select: text;
  }

  .error {
    margin: 0 0 var(--sp-4);
    color: var(--status-delete);
  }

  /* Raw Git output is never reformatted or truncated (INV-05). */
  .raw {
    margin: 0;
    padding: var(--sp-5);
    background: var(--surface-input);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    white-space: pre;
    overflow: auto;
  }

  .subject {
    margin: 0 0 var(--sp-4);
    font-weight: 600;
  }

  .body {
    margin: 0 0 var(--sp-5);
    font-family: inherit;
    white-space: pre-wrap;
    color: var(--text-secondary);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--sp-3) var(--sp-6);
    margin: 0 0 var(--sp-5);
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
