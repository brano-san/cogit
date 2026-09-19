<script lang="ts">
  import Panel from "$components/layout/Panel.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import StatusBar from "$components/layout/StatusBar.svelte";
  import Toolbar from "$components/layout/Toolbar.svelte";
  import { layout } from "$stores/layout.svelte";
  import { getAppInfo, type AppInfo } from "$lib/ipc";

  /**
   * Walking skeleton (doc/modules/M0-skeleton.md).
   *
   * Panels are intentionally empty: M0 exists to prove the
   * Rust → specta → TypeScript → Svelte → window chain is wired, nothing more.
   */

  let info = $state<AppInfo | null>(null);
  let ipcError = $state<string | null>(null);

  $effect(() => {
    getAppInfo()
      .then((result) => {
        info = result;
      })
      .catch((err: unknown) => {
        ipcError = err instanceof Error ? err.message : String(err);
      });
  });

  const fractions = $derived(layout.fractions);
</script>

<div class="app">
  <Toolbar />

  <div class="workspace">
    <div class="left-column" style:flex="0 0 {fractions.leftColumn * 100}%">
      <div class="pane" style:flex="0 0 {fractions.repositories * 100}%">
        <Panel title="Repositories" empty="No repositories yet — M3 fills this panel." />
      </div>
      <Splitter
        direction="horizontal"
        value={fractions.repositories}
        label="Resize repositories panel"
        onchange={(d) => layout.nudge("repositories", d)}
        onreset={() => layout.resetOne("repositories")}
      />
      <div class="pane grow">
        <Panel title="References" empty="Branches, tags and stashes arrive in M5." />
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
          <div class="skeleton-note">
            {#if ipcError}
              <p class="error">IPC call failed: {ipcError}</p>
            {:else if info}
              <p>
                Walking skeleton is live. <code class="mono">app_info</code> answered over
                generated bindings.
              </p>
              <dl>
                <dt>Version</dt>
                <dd class="mono">{info.version}</dd>
                <dt>Build</dt>
                <dd class="mono">{info.debugBuild ? "debug" : "release"}</dd>
                <dt>Log file</dt>
                <dd class="mono">{info.logPath}</dd>
              </dl>
            {:else}
              <p class="muted">Calling the backend…</p>
            {/if}
          </div>
        </Panel>
      </div>
    </div>
  </div>

  <StatusBar
    repository="No repository"
    branch="—"
    summary="Walking skeleton"
    version={info?.version}
    status={ipcError ? "IPC error" : "Ready"}
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
    gap: 0;
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

  .skeleton-note {
    padding: var(--sp-6);
    font-size: var(--fs-dense);
    user-select: text;
  }

  .skeleton-note p {
    margin: 0 0 var(--sp-5);
  }

  .error {
    color: var(--status-delete);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--sp-3) var(--sp-6);
    margin: 0;
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
