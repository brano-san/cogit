<script lang="ts">
  import { ask, open as openFolderDialog } from "@tauri-apps/plugin-dialog";

  import BranchList from "$components/branch-tree/BranchList.svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import FileList from "$components/file-list/FileList.svelte";
  import CommitList from "$components/graph/CommitList.svelte";
  import GraphFilter from "$components/graph/GraphFilter.svelte";
  import Panel from "$components/layout/Panel.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import StatusBar from "$components/layout/StatusBar.svelte";
  import Toolbar from "$components/layout/Toolbar.svelte";
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import { formatCommitDate, shortOid } from "$lib/format";
  import { getAppInfo, type AppInfo } from "$lib/ipc";
  import { commit } from "$stores/commit.svelte";
  import { worktree } from "$stores/worktree.svelte";
  import { diff } from "$stores/diff.svelte";
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

  function openDiff(path: string) {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (id && oid) void diff.load(id, oid, path);
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

<div class="app">
  <Toolbar busy={repository.busy ? "Opening repository…" : undefined} />

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
              <BranchList title="Local Branches" branches={repository.localBranches} />
              <BranchList title="Remote" branches={repository.remoteBranches} />
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
            {#if onWorkingTree}
              <FileList
                sections={[
                  {
                    title: "Staged",
                    files: worktree.staged,
                    actions: [{ label: "Unstage", title: "Unstage", run: unstage }],
                  },
                  {
                    title: "Unstaged",
                    files: worktree.unstaged,
                    actions: [
                      { label: "Stage", title: "Stage", run: stage },
                      { label: "Discard", title: "Discard changes", run: discard },
                    ],
                  },
                ]}
                empty="The working tree is clean."
                selected={diff.path}
              />
            {:else}
              <FileList
                sections={[{ files: commit.files }]}
                empty="Select a commit to see the files it changed."
                selected={diff.path}
                onselect={openDiff}
              />
            {/if}
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

  <StatusBar
    repository={repo?.name ?? "No repository"}
    branch={repo ? repository.headLabel : undefined}
    summary={repo ? `${graph.rows.length} commits · ${repo.branches.length} refs` : "Milestone C"}
    version={info?.version}
    status={repository.error ? "Error" : repository.busy ? "Working…" : "Ready"}
  />
</div>

<style>
  .app {
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
