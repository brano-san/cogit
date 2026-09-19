<script lang="ts">
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";

  import BranchList from "$components/branch-tree/BranchList.svelte";
  import Panel from "$components/layout/Panel.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import StatusBar from "$components/layout/StatusBar.svelte";
  import Toolbar from "$components/layout/Toolbar.svelte";
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import { getAppInfo, type AppInfo } from "$lib/ipc";
  import { layout } from "$stores/layout.svelte";
  import { repository } from "$stores/repository.svelte";

  /**
   * Milestone B (doc/00-roadmap.md): one thin slice all the way through —
   * `gix` → Tauri command → generated bindings → panel. The remaining panels stay
   * empty until their own modules land.
   */

  let info = $state<AppInfo | null>(null);

  $effect(() => {
    getAppInfo().then((result) => {
      info = result;
    });
  });

  const fractions = $derived(layout.fractions);
  const repo = $derived(repository.current);

  async function pickRepository() {
    const picked = await openFolderDialog({ directory: true, title: "Open Repository" });
    if (typeof picked === "string") {
      await repository.open(picked);
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
          <Panel title="Graph &amp; History" empty="The commit DAG is built in M4." />
        </div>
        <Splitter
          direction="vertical"
          value={fractions.graph}
          label="Resize graph panel"
          onchange={(d) => layout.nudge("graph", d)}
          onreset={() => layout.resetOne("graph")}
        />
        <div class="pane grow">
          <Panel title="Files" count={0} empty="Changed files appear in M6." />
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
          <div class="detail">
            {#if repository.error}
              <p class="error">{repository.error.message}</p>
              {#if repository.error.isCommandFailure && repository.error.detail.kind === "command"}
                <pre class="raw">{repository.error.detail.data.stderr}</pre>
              {/if}
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
              <p class="muted">Side-by-side diffs arrive in M7.</p>
            {:else}
              <p class="muted">Open a repository to begin.</p>
            {/if}
          </div>
        </Panel>
      </div>
    </div>
  </div>

  <StatusBar
    repository={repo?.name ?? "No repository"}
    branch={repo ? repository.headLabel : undefined}
    summary={repo ? `${repo.branches.length} refs` : "Milestone B"}
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
