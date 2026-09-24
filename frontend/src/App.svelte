<script lang="ts">
  import { tick, untrack } from "svelte";
  import { ask, message as dialogMessage, open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { checkForUpdates, message, type UpdateOutcome } from "$lib/updates";
  import { leaveRepositoryDialogs } from "$lib/leaving";
  import { retryOf } from "$lib/retry";
  import { branchNameProblem, optional, textProblem } from "$lib/names";
  import { finder } from "$stores/finder.svelte";
  import { THIRD_PARTY_FILE } from "$lib/third-party";

  import DiffPanel from "$components/panels/DiffPanel.svelte";
  import ReferencesPanel from "$components/panels/ReferencesPanel.svelte";
  import RefSortButtons from "$components/branch-tree/RefSortButtons.svelte";
  import SafetyJournal from "$components/layout/SafetyJournal.svelte";
  import RepositoriesPanel from "$components/panels/RepositoriesPanel.svelte";
  import GraphPanel from "$components/panels/GraphPanel.svelte";
  import FilesPanel from "$components/panels/FilesPanel.svelte";
  import CommitPanel from "$components/panels/CommitPanel.svelte";
  import CommitDetailsPane from "$components/panels/CommitDetailsPane.svelte";
  import SplitOffDialog from "$components/file-list/SplitOffDialog.svelte";
  import GraphFilter from "$components/graph/GraphFilter.svelte";
  import RebaseEditor from "$components/graph/RebaseEditor.svelte";
  import DropMenu from "$components/layout/DropMenu.svelte";
  import Panel from "$components/layout/Panel.svelte";
  import CommandPalette from "$components/layout/CommandPalette.svelte";
  import AboutDialog from "$components/common/AboutDialog.svelte";
  import ExitDialog from "$components/common/ExitDialog.svelte";
  import ConfigEditor from "$components/common/ConfigEditor.svelte";
  import Notifications from "$components/layout/Notifications.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import { exitRows, type ExitAction } from "$lib/exit";
  import { exitFlow } from "$stores/exit.svelte";
  import { describeSkipped } from "$lib/skipped";
  import { health } from "$stores/health.svelte";
  import SettingsPanel from "$components/layout/SettingsPanel.svelte";
  import HooksPanel from "$components/layout/HooksPanel.svelte";
  import FindObject from "$components/layout/FindObject.svelte";
  import CommandOutput from "$components/layout/CommandOutput.svelte";
  import { suppressNativeMenu } from "$lib/native-menu";
  import { footerRepository, panelView } from "$lib/repo-phase";
  import { startTracing, timed, trace } from "$lib/trace";
  import OutputPanel from "$components/layout/OutputPanel.svelte";
  import StateBanner from "$components/layout/StateBanner.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import StatusBar from "$components/layout/StatusBar.svelte";
  import Toolbar from "$components/layout/Toolbar.svelte";
  import ScanDialog from "$components/repo-tree/ScanDialog.svelte";
  import PromptDialog from "$components/layout/PromptDialog.svelte";
  import StashDialogs from "$components/layout/StashDialogs.svelte";
  import RemoteOpsDialog from "$components/remote/RemoteOpsDialog.svelte";
  import RepoSettingsDialog from "$components/remote/RepoSettingsDialog.svelte";
  import { remoteCommands, submoduleScope } from "$lib/remote-menu";
  import { remoteOps } from "$stores/remote-ops.svelte";
  import RemoveFilesDialog from "$components/file-list/RemoveFilesDialog.svelte";
  import IndexEditorDialog from "$components/file-list/IndexEditorDialog.svelte";
  import { openInvestigate } from "$lib/investigate/open";
  import WorktreesPanel from "$components/panels/WorktreesPanel.svelte";
  import AddWorktreeDialog from "$components/repo-tree/AddWorktreeDialog.svelte";
  import RemoveWorktreeDialog from "$components/repo-tree/RemoveWorktreeDialog.svelte";
  import { branchChoices, hasStale, removable } from "$lib/worktree-list";
  import { fileFormat, shortOid } from "$lib/format";
  import { checkedIds, disabledIds, type PaletteCommand } from "$lib/palette";
  import { reasonFor, type Context } from "$lib/availability";
  import { refAt, splitMarked, targetsOf, type MenuContext, type ToolbarFacts } from "$lib/toolbar";
  import { currentRemote, remotePlan, syncSteps, type SyncOrder } from "$lib/toolbar-prefs";
  import { toolbar } from "$stores/toolbar.svelte";
  import { stashDialog } from "$stores/stash-dialog.svelte";
  import { allowsSelectAll, settle, step } from "$lib/panel-focus";
  import { pullRequestUrl } from "$lib/pull-request";
  import { commitScope } from "$lib/commit-scope";
  import { activity, applyOperation } from "$lib/operations";
  import { measurer } from "$lib/timing";
  import { refMenu } from "$lib/context-menu";
  import RefActions from "$components/menus/RefActions.svelte";
  import { compareView } from "$stores/compare-view.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { commitFileMenu, worktreeFileMenu } from "$lib/file-menu";
  import { fileName, runFileMenuCommand, type FileActions, type FileScope } from "$lib/file-actions";
  import { listedMessage } from "$lib/file-dialogs";
  import * as fileMenus from "$lib/ipc/file-menus";
  import { desktop } from "$stores/desktop.svelte";
  import { groupChoices, parseRepoCommand, repoMenu } from "$lib/repo-menu";
  import type { ListedRepo } from "$lib/repo-list";
  import { UNGROUPED } from "$lib/repo-groups";
  import { repoList } from "$stores/repo-list.svelte";
  import { compareUrl } from "$lib/compare-params";
  import { dropActions, type DropAction, type DragPayload } from "$lib/drop-target";
  import { moveEntry } from "$lib/rebase-plan";
  import { stateBanner, type BannerAction } from "$lib/repo-state";
  import { blockedByLocalChanges } from "$lib/checkout-refusal";
  import { capFraction, floorFraction, PANELS, type PanelId } from "$lib/perspectives";
  import { graphPanelMinWidth } from "$lib/graph-panel";
  import { repoClick } from "$lib/repo-click";
  import { browserSources, start as startMemoryProbe } from "$lib/mem-probe";
  import { liveListeners } from "$lib/listener-count";
  import type { Settings } from "$lib/settings";
  import {
    checkout,
    CogitError,
    abortOperation,
    continueOperation,
    createBranch,
    getAppInfo,
    openThirdPartyLicences,
    addToGitignore,
    closeThisWindow,
    interactiveRebase,
    isPublished,
    deleteMergedBranches,
    protectingRefs,
    rebaseProgress,
    rebaseTodo,
    rollbackTo,
    splitOff,
    mergeInto,
    stashSelection,
    fetchRemote,
    listRemotes,
    onMenuCommand,
    openInTerminal,
    runCheck,
    terminalChoices,
    openRepository,
    openSubmodule,
    openWorktree,
    listOperations,
    readGitConfig,
    writeGitConfig,
    reportMemory,
    reportTiming,
    commitTemplate,
    stageMode,
    openMergeWindow,
    openCompareWindow,
    popupContextMenu,
    resolveConflict,
    setMenuState,
    rebaseOnto,
    skipOperation,
    type AppInfo,
    type Branch,
    type RepoId,
    type Tag,
  } from "$lib/ipc";
  import { openBlame } from "$lib/blame-window";
  import { commit } from "$stores/commit.svelte";
  import { conflicts } from "$stores/conflicts.svelte";
  import { worktree } from "$stores/worktree.svelte";
  import { worktrees } from "$stores/worktrees.svelte";
  import { diff } from "$stores/diff.svelte";
  import { errors } from "$stores/errors.svelte";
  import { notices } from "$stores/notices.svelte";
  import { FETCH_MODULES } from "$lib/health";
  import { output } from "$stores/output.svelte";
  import { network } from "$stores/network.svelte";
  import { recovery } from "$stores/recovery.svelte";
  import { safety } from "$stores/safety.svelte";
  import { stashes } from "$stores/stashes.svelte";
  import { submodules } from "$stores/submodules.svelte";
  import { graph } from "$stores/graph.svelte";
  import { hooks } from "$stores/hooks.svelte";
  import { avatars } from "$stores/avatars.svelte";
  import { prompt } from "$stores/prompt.svelte";
  import { session } from "$stores/session.svelte";
  import { flow } from "$stores/flow.svelte";
  import { droppedRepositories } from "$lib/drop-open";
  import { connect } from "$lib/wiring";
  import { planFor } from "$lib/disk-change";
  import { clear as freshen, mark as markStale } from "$lib/staleness";
  import { unsavedSummary } from "$lib/unsaved";
  import { overlap } from "$stores/overlap.svelte";
  import { settings } from "$stores/settings.svelte";
  import { layout } from "$stores/layout.svelte";
  import { repoGroups } from "$stores/repo-groups.svelte";
  import { repository } from "$stores/repository.svelte";
  import { filesView } from "$stores/files-view.svelte";
  import { scan } from "$stores/scan.svelte";
  import { refs } from "$stores/refs.svelte";
  import { stashView } from "$stores/stash-view.svelte";
  import { buildRefTree, visibleTips, type RefNode } from "$lib/ref-nodes";

  /** Settings the open diff was computed with: changing one has to re-run it. */
  const REDIFF: readonly (keyof Settings)[] = [
    "algorithm",
    "contextLines",
    "wordDiff",
    "detectMoves",
  ];

  const PANEL_TITLES: Record<PanelId, string> = {
    repositories: "Repositories",
    refs: "References",
    graph: "Graph",
    files: "Files",
    commit: "Commit Message",
    diff: "Diff",
    worktrees: "Worktrees",
  };

  const measure = measurer((label, ms, detail) => void reportTiming(label, ms, detail));

  /** Maximising acts on the panel the pointer last entered; there is no focus ring yet. */
  let focused = $state<PanelId>("graph");

  $effect(() => {
    const landed = settle(focused, (panel) => layout.visible(panel));
    if (landed !== focused) focused = landed;
  });
  let running = $state.raw<Map<number, string>>(new Map());
  let info = $state<AppInfo | null>(null);
  /** The last update check this session, for the line in About. */
  let lastUpdate = $state.raw<UpdateOutcome | null>(null);
  let opening = $state(false);
  let scanOpen = $state(false);
  let addWorktreeOpen = $state(false);
  let worktreeRemoval = $state.raw<{
    entry: import("$lib/ipc").WorktreeEntry;
    changes: import("$lib/ipc").FileEntry[] | null;
  } | null>(null);
  let markedFiles = $state.raw<string[]>([]);
  type RepoMenuSubject =
    | { kind: "repository"; root: string; overview: import("$lib/ipc").RepoOverview | null }
    | { kind: "submodule"; root: string; row: import("$lib/module-tree").ModuleRow };
  let repoTarget = $state.raw<RepoMenuSubject | null>(null);
  let terminals = $state.raw<{ id: string; label: string }[]>([]);
  let groupTarget = $state<string | null>(null);
  /** Remembered per repository: a check command is worth typing once, not once per pause. */
  let checkCommand = $state("");
  let checkVerdict = $state.raw<import("$lib/ipc").HookRun | null>(null);
  let checking = $state(false);
  let markedRepos = $state.raw<string[]>([]);
  let bulk = $state.raw<import("$lib/operations").BulkProgress | undefined>(undefined);
  let journalOpen = $state(false);
  let journalBusy = $state(false);
  let paletteOpen = $state(false);
  let settingsOpen = $state(false);
  let repoSettingsOpen = $state(false);
  let finderOpen = $state(false);
  let recentCommands = $state<string[]>([]);
  let dropMenu = $state.raw<{
    actions: DropAction[];
    source: DragPayload;
    target: DragPayload;
    x: number;
    y: number;
  } | null>(null);
  let progress = $state.raw<import("$lib/ipc").RebaseProgress | null>(null);
  let rebaseOpen = $state(false);
  let rebaseBase = $state("");
  let rebasePlan = $state.raw<import("$lib/ipc").TodoEntry[]>([]);
  let rebaseBusy = $state(false);
  let rebasePaused = $state(false);
  let template = $state<string | null>(null);
  let splitOpen = $state(false);
  let splitPublished = $state(false);
  /** Which shared branches hold a commit, once asked. Answering costs a graph walk per
      remote ref, so it is asked when the user opens a menu, not on every selection. */
  let protection = $state.raw<ReadonlyMap<string, readonly string[]>>(new Map());
  const protectedBy = $derived(protection.get(commit.oid ?? "") ?? []);

  /** Help ▸ Check for Updates, and the start-up check when the setting is on. The
      plugin is loaded on demand: nobody pays for the updater until it is wanted. */
  async function runUpdateCheck(quiet = false): Promise<void> {
    const [{ check }, { relaunch }] = await Promise.all([
      import("@tauri-apps/plugin-updater"),
      import("@tauri-apps/plugin-process"),
    ]);
    lastUpdate = await checkForUpdates(
      {
        check: () => check(),
        relaunch,
        confirm: (outcome) => ask(message(outcome), { title: "Check for Updates", kind: "info" }),
        report: (text) => void dialogMessage(text, { title: "Check for Updates", kind: "info" }),
      },
      { quiet },
    );
  }

  async function learnProtection(oid: string): Promise<readonly string[]> {
    const id = repository.current?.repo;
    if (!id) return [];
    const known = protection.get(oid);
    if (known) return known;
    const refs = await protectingRefs(id, oid).catch(() => [] as string[]);
    protection = new Map(protection).set(oid, refs);
    return refs;
  }
  /** Panels a disk event has outdated; cleared as each reload lands. */
  let stale = $state.raw<ReadonlySet<PanelId>>(new Set());
  let splitBusy = $state(false);
  const pointer = { x: 0, y: 0 };
  let refFilter = $state("");
  let dropping = $state(false);
  let fileMask = $state("");

  /** Startup, not a reaction: everything here runs once.

      Wrapped in `untrack` as a whole rather than read by read. `activate()` writes the
      session back, and anything this effect touches that the session feeds — including
      `session.repositories`, which `repository.restore()` reads for itself — makes the
      effect depend on its own write and re-enter `open_repository` about a hundred times
      a second (R-93). Untracking one getter at a time only moves the problem to the next
      caller down. */
  // D4: leaving a commit drops the file that was open in it, so the tick in Files and the
  // panel beside it always agree (doc/12-risks.md, R-138).
  commit.onchange = () => {
    diff.clear();
    compareView.clear();
    // A stash outranks a commit in Files; picking a commit in the graph leaves it.
    stashView.clear();
  };

  $effect(() => {
    untrack(() => {
      startTracing();
      trace("startup", "the window is running");
      // Nothing in a Git client is a web page (R-127).
      suppressNativeMenu(document);
      getAppInfo()
        .then((result) => (info = result))
        .catch((err) => errors.report(err, "Could not read the application info"));
      void remoteOps.detectLfs();
      void settings.load().then(() => {
        diff.whitespace = settings.current.ignoreWhitespace;
        // Only after the settings are read: the tick is what permits the network call.
        if (settings.current.autoUpdate) void runUpdateCheck(true);
      });
      void settings.loadBindings();
      void health.loadIgnored();
      void toolbar.load();
      void terminalChoices().then((found) => (terminals = found));
      const wanted = session.active;
      const remembered = wanted === null ? null : session.selected(wanted);
      void timed("startup", "restore the session", () => repository.restore()).then(() => {
        const back =
          repository.openRepos.find((entry) => entry.root === wanted) ?? repository.openRepos[0];
        if (back) void activate(back.root, back.root === wanted ? remembered : null);
      });
    });
  });

  const fractions = $derived(layout.fractions);
  /** `--commit-panel-min` plus the splitter, in the same CSS pixels. */
  const COMMIT_MIN_PX = 126;
  let filesColumnHeight = $state(0);
  /** `--worktrees-panel-min` plus the splitter. */
  const WORKTREES_MIN_PX = 98;
  let reposColumnHeight = $state(0);
  const graphMin = graphPanelMinWidth();
  let graphPane = $state<HTMLDivElement | null>(null);
  let topRowWidth = $state(0);
  let workspaceWidth = $state(0);
  /** The Graph panel's CSS minimum in pixels, so the splitters stop where the panel does. */
  const graphMinPx = () => (graphPane ? parseFloat(getComputedStyle(graphPane).minWidth) || 0 : 0);
  const shown = $derived({
    repositories: layout.visible("repositories"),
    refs: layout.visible("refs"),
    graph: layout.visible("graph"),
    files: layout.visible("files"),
    commit: layout.visible("commit"),
    diff: layout.visible("diff"),
    worktrees: layout.visible("worktrees"),
  });
  const reposColumn = $derived(shown.repositories || shown.worktrees);
  const leftColumn = $derived(reposColumn || shown.refs);
  const repo = $derived(repository.current);
  const lastUndo = $derived(safety.lastFor(repo?.repo ?? null));
  /** One field for every panel that depends on an open repository. The panel decides
      from it both what its header counts and what its body says (R-119). */
  const panelState = $derived(panelView(repository.phase));
  /** The banner belongs to whatever repository the panels are showing, and a submodule
      opened from the tree is a different case from one the user checked out (R-130). */
  const banner = $derived(
    repo ? stateBanner(repo.state, repo.indexLock, submodules.open !== null) : null,
  );
  const tracked = $derived(repository.localBranches.find((b) => b.isHead));
  /** The staged rows the Files list shows; null until it has said. */
  let shownStaged = $state.raw<string[] | null>(null);
  const scope = $derived(commitScope(worktree.staged, fileMask, shownStaged));
  const prUrl = $derived.by(() => {
    const head = tracked?.name;
    const base = tracked?.upstream?.split("/").slice(1).join("/") ?? "main";
    if (!network.url || !head) return null;
    return pullRequestUrl(network.url, base, head, commit.details?.summary ?? head);
  });

  const onWorkingTree = $derived(repo !== undefined && repo !== null && commit.oid === null);
  const filesColumn = $derived(shown.files || (shown.commit && onWorkingTree));
  const topRow = $derived(shown.graph || filesColumn);

  $effect(() => {
    void commit.oid;
    diff.clear();
  });

  /** The id, not the object: a status refresh replaces `repository.current`, and reading
      the object here reloaded the working tree after every mutation that had just done so. */
  const currentRepo = $derived(repository.current?.repo);
  $effect(() => {
    const id = currentRepo;
    if (id && commit.oid === null) void worktree.load(id);
  });

  // Every failure that carries raw Git output goes to the dialog; INV-05 says the user
  // sees exactly what Git said, not a summary of it.
  $effect(() => errors.report(worktree.error, "Could not read the working tree"));
  $effect(() => errors.report(repository.error, "Could not load the repository"));
  $effect(() => errors.report(commit.error, "Could not load the commit"));
  $effect(() => errors.report(diff.error, "Could not show the diff"));
  $effect(() => errors.report(graph.error, "Could not load the graph"));
  $effect(() => errors.report(hooks.error, hooks.failure ?? "Could not read the hooks"));
  $effect(() => errors.report(compareView.error, "Could not compare the commits"));
  $effect(() => errors.report(stashView.error, "Could not open the stash"));

  /** Refreshed with the rest of the state after every mutation, so the stack follows
      Continue and Abort; a reactive version fired on each loading toggle. */
  async function refreshProgress() {
    const id = repository.current?.repo;
    const epoch = repository.epoch;
    const found = id ? await rebaseProgress(id).catch(() => null) : null;
    if (repository.epoch === epoch) progress = found;
  }

  /** Every change to the working tree ends the same way: reload it and refresh what
      depends on it, or report why not. `false` means nothing was done. */
  async function mutate(
    step: (repo: import("$lib/ipc").RepoId) => Promise<unknown>,
    paths: string[] = [],
  ): Promise<boolean> {
    const id = repository.current?.repo;
    if (!id) return false;
    const epoch = repository.epoch;
    try {
      await step(id);
    } catch (err) {
      errors.report(err, "Could not change the working tree");
      return false;
    }
    if (repository.epoch !== epoch) return true;
    await worktree.load(id);
    await afterMutation(paths);
    return true;
  }

  /** For a change `mutate` did not make: resolving a conflict, an Undo from the journal,
      a submodule update. `afterMutation` leaves the working tree to its callers, and the
      watcher is quiet right after our own writes, so the file list would stay as it was. */
  async function afterWorkingTreeChange(paths: string[] = []) {
    const id = repository.current?.repo;
    if (id) await worktree.load(id);
    await afterMutation(paths);
  }

  async function afterMutation(paths: string[] = []) {
    diff.dropIfAffected(paths);
    await repository.refreshStatus();
    const id = repository.current?.repo;
    await Promise.all([
      id ? stashes.refresh(id) : Promise.resolve(),
      id ? network.refresh(id) : Promise.resolve(),
      id ? recovery.refresh(id) : Promise.resolve(),
      submodules.refresh(),
      id ? conflicts.refresh(id) : Promise.resolve(),
      output.refreshProblems(),
      safety.refresh(),
      refreshProgress(),
      loadTemplate(),
      output.open ? output.refresh() : Promise.resolve(),
    ]);
  }

  /** What every command rule reads (issue 2). One object, one definition. */
  const commands = $derived<Context>({
    repository: repo !== null,
    remote: Boolean(network.primary),
    selection: markedFiles.length > 0,
    changes: worktree.total > 0,
    staged: worktree.staged.length > 0,
    commit: commit.oid !== null,
    branch: tracked !== undefined,
    undo: lastUndo !== null,
    file: diff.path !== null,
  });

  const headOid = $derived(repo && repo.head.kind !== "unborn" ? repo.head.oid : null);

  $effect(() => {
    void toolbar.checkMerged(currentRepo, commit.oid, headOid);
  });

  /** What the toolbar's rules read; recomputed on every selection change (task #31). */
  const toolbarFacts = $derived<ToolbarFacts>({
    repository: repo !== null,
    remote: Boolean(network.primary),
    onWorkingTree: onWorkingTree && stashView.contents === null,
    ...splitMarked({ marked: markedFiles, unstaged: worktree.unstaged, staged: worktree.staged }),
    unstaged: worktree.unstaged.map((file) => file.path),
    staged: worktree.staged.map((file) => file.path),
    commit: commit.oid,
    head: headOid,
    branch: repo?.head.kind === "branch",
    merged: toolbar.merged,
    stashes: stashes.entries.length,
    undo: lastUndo !== null,
  });

  const pullRemote = $derived(currentRemote(tracked?.upstream, network.remotes));

  const toolbarMenus = $derived<MenuContext>({
    remotes: network.remotes,
    current: pullRemote,
    prefs: toolbar.prefs,
  });

  /** What Remote ▸ LFS ▸ Lock and Submodule act on: the ticked files, or the one in Diff. */
  const pickedFiles = $derived(markedFiles.length > 0 ? markedFiles : diff.path ? [diff.path] : []);
  const remoteActions = remoteOps.actions({
    files: () => pickedFiles,
    changed: afterRefChange,
    synchronize: () => void syncNow(),
    repoSettings: () => (repoSettingsOpen = true),
  });

  const palette = $derived.by<PaletteCommand[]>(() => {
    const noRepo = reasonFor({ repository: true }, commands);
    const noRemote = reasonFor({ remote: true }, commands);
    const nothingStaged = reasonFor({ staged: true }, commands);

    return [
      { id: "open", title: "Open Repository…", run: () => void pickRepository() },
      { id: "fetch", title: "Fetch", unavailable: noRepo ?? noRemote, run: () => void runNetwork("fetch") },
      { id: "pull", title: "Pull", unavailable: noRepo ?? noRemote, run: () => void pullNow() },
      { id: "push", title: "Push", unavailable: noRepo ?? noRemote, run: () => void runNetwork("push") },
      {
        id: "stash",
        title: "Stash All",
        shortcut: "Ctrl+S",
        synonyms: ["shelve"],
        unavailable: noRepo,
        run: stashAll,
      },
      {
        id: "stash-selection",
        title: "Stash Selection",
        shortcut: "Ctrl+Alt+S",
        synonyms: ["shelve some"],
        unavailable: noRepo ?? (markedFiles.length > 0 ? undefined : "No file is ticked"),
        run: () => void stashSelected(),
      },
      { id: "tag", title: "Create Tag", unavailable: noRepo, run: () => void refActions?.addTag(null) },
      { id: "commit", title: "Commit Staged", unavailable: noRepo ?? nothingStaged, run: () => {} },
      {
        id: "undo",
        title: "Undo Last Operation",
        unavailable: lastUndo ? undefined : "Nothing to undo",
        run: () => void undo(),
      },
      { id: "output", title: "Toggle Output Panel", shortcut: "Ctrl+Shift+7", run: () => output.toggle() },
      {
        id: "copy-path",
        title: "Copy the File Path",
        unavailable: diff.path ? undefined : "No file is open in the Diff panel",
        run: () => void copyText(diff.path ?? ""),
      },
      {
        id: "copy-branch",
        title: "Copy the Branch Name",
        unavailable: tracked ? undefined : "HEAD is not on a branch",
        run: () => void copyText(tracked?.name ?? ""),
      },
      {
        id: "copy-sha",
        title: "Copy the Commit SHA",
        unavailable: commit.oid ? undefined : "Select a commit first",
        run: () => void copyText(commit.oid ?? ""),
      },
      {
        id: "rebase-i",
        title: "Rebase Commits After This One…",
        synonyms: ["interactive rebase", "squash", "reorder"],
        unavailable: commit.oid ? undefined : "Select a commit first",
        run: () => void openRebase(),
      },
      {
        id: "flow-init",
        title: "Git-Flow: Set Up",
        synonyms: ["gitflow", "develop branch"],
        unavailable: noRepo ?? (flow.status.initialised ? "Already set up" : undefined),
        run: () => void runFlow(() => flow.init(repo!.repo)),
      },
      ...(["feature", "release", "hotfix"] as const).map((kind) => ({
        id: `flow-${kind}`,
        title: `Git-Flow: Start ${kind[0]?.toUpperCase()}${kind.slice(1)}…`,
        synonyms: ["gitflow", kind],
        unavailable: noRepo ?? (flow.status.initialised ? undefined : "Set up Git-Flow first"),
        run: () => askFlowStart(kind),
      })),
      {
        id: "flow-finish",
        title: "Git-Flow: Finish This Branch…",
        synonyms: ["gitflow", "merge back"],
        unavailable: flow.current ? undefined : "Not on a Git-Flow branch",
        run: () => askFlowFinish(),
      },
      {
        id: "split-off",
        title: "Split Off Files…",
        synonyms: ["split commit", "surgery"],
        unavailable: !commit.oid
          ? "Select a commit first"
          : protectedBy.length > 0
            ? `Already on ${protectedBy.join(", ")}`
            : undefined,
        run: () => void openSplit(),
      },
      {
        id: "rollback",
        title: "Roll Back Tree To This Commit",
        synonyms: ["restore", "revert files"],
        unavailable: commit.oid ? undefined : "Select a commit first",
        run: () => void rollbackFiles([]),
      },
      {
        id: "close",
        title: "Close Repository",
        shortcut: "Ctrl+W",
        unavailable: noRepo,
        run: () => void closeCurrent(),
      },
      { id: "refresh", title: "Refresh", shortcut: "F5", unavailable: noRepo, run: () => void repository.refresh() },
      {
        id: "branch",
        title: "New Branch…",
        unavailable: noRepo,
        run: () => void runBannerAction("createBranch"),
      },
      { id: "reset-layout", title: "Reset Perspective", run: () => layout.reset() },
      {
        id: "overlap",
        title: "Toggle Commit Overlap Column",
        synonyms: ["who else touched", "conflict risk"],
        run: () => overlap.toggle(),
      },
      {
        id: "avatars",
        title: "Toggle Author Avatars",
        synonyms: ["gravatar", "pictures", "faces"],
        run: () =>
          void settings.apply({
            ...settings.current,
            avatars: settings.current.avatars === "gravatar" ? "off" : "gravatar",
          }),
      },
      {
        id: "hooks",
        title: "Manage Hooks…",
        synonyms: ["pre-commit", "git hooks"],
        unavailable: noRepo,
        run: () => {
          const id = repository.current?.repo;
          if (id) void hooks.show(id);
        },
      },
      {
        id: "perspective-main",
        title: "Perspective: Main",
        run: () => layout.switch("main"),
      },
      {
        id: "perspective-review",
        title: "Perspective: Review",
        run: () => layout.switch("review"),
      },
      {
        id: "maximize-panel",
        title: "Maximise Panel",
        shortcut: "Shift+F11",
        synonyms: ["zoom", "full screen panel"],
        run: () => layout.toggleMaximized(focused),
      },
      {
        id: "worktree-add",
        title: "Add Worktree…",
        synonyms: ["worktree", "second checkout"],
        unavailable: noRepo,
        run: () => (addWorktreeOpen = true),
      },
      {
        id: "worktree-remove",
        title: "Remove Worktree…",
        unavailable:
          noRepo ??
          (removable(worktrees.current)
            ? undefined
            : "Select a worktree other than the main one in the Worktrees panel"),
        run: () => {
          const entry = worktrees.current;
          if (removable(entry)) void removeWorktreeAt(entry);
        },
      },
      {
        id: "worktree-prune",
        title: "Prune Obsolete Worktrees…",
        synonyms: ["worktree prune", "missing worktree"],
        unavailable: noRepo ?? (hasStale(worktrees.entries) ? undefined : "No worktree is missing"),
        run: () => void pruneWorktreesHere(),
      },
      ...PANELS.map((panel) => ({
        id: `panel-${panel}`,
        title: `Toggle ${PANEL_TITLES[panel]} Panel`,
        run: () => layout.togglePanel(panel),
      })),
      {
        id: "palette",
        title: "Find Command",
        shortcut: "Ctrl+Shift+P",
        run: () => {
          if (commit.oid) void learnProtection(commit.oid);
          paletteOpen = true;
        },
      },
      {
        id: "check-updates",
        title: "Check for Updates",
        run: () => void runUpdateCheck(),
      },
      {
        id: "edit-config-repository",
        title: "Edit Git Config: Repository",
        synonyms: ["config", ".git/config", "settings", "remote"],
        unavailable: noRepo,
        run: () => void openConfig("repository"),
      },
      {
        id: "edit-config-user",
        title: "Edit Git Config: User",
        synonyms: ["config", "gitconfig", "global", "user.name", "email"],
        run: () => void openConfig("user"),
      },
      {
        id: "exit",
        title: "Exit",
        shortcut: "Alt+X",
        synonyms: ["quit", "close"],
        run: () => {
          // No window is being closed by hand, so the dialog must not say one is.
          exitFlow.fromCommand();
          // Closed in Rust (R-86): the webview is not allowed `window.close`, and the refusal
          // was silent — Exit from the menu, Alt+X and the palette did nothing (R-190).
          void closeThisWindow().catch(() => exitFlow.takeSource());
        },
      },
      {
        id: "about",
        title: "About Cogit",
        run: () => (aboutOpen = info !== null),
      },
      {
        id: "settings",
        title: "Preferences",
        shortcut: "Ctrl+,",
        synonyms: ["settings", "options", "customise toolbar", "toolbar buttons"],
        run: () => openSettings(),
      },
      {
        id: "find",
        title: "Find Object",
        shortcut: "Ctrl+P",
        synonyms: ["goto", "jump"],
        unavailable: noRepo,
        run: () => (finderOpen = true),
      },
      {
        id: "pr",
        title: "Create Pull Request",
        synonyms: ["merge request", "pr", "mr"],
        unavailable: prUrl ? undefined : "No GitHub, GitLab or Bitbucket remote",
        run: () => void openPullRequest(),
      },
      {
        id: "scan",
        title: "Scan Folder for Repositories",
        synonyms: ["discover", "find repositories", "import"],
        run: () => {
          scanOpen = true;
          void browseForScan();
        },
      },
      {
        id: "journal",
        title: "Safety Journal",
        synonyms: ["undo history", "recover", "what did I just do"],
        unavailable: noRepo,
        run: () => {
          journalOpen = true;
          void safety.refresh();
        },
      },
      {
        id: "fetch-all",
        title: "Fetch All",
        synonyms: ["update every repository"],
        unavailable: repository.openRepos.length > 0 ? undefined : "No repository is open",
        run: () => void fetchAll(),
      },
      {
        id: "reveal-log",
        title: "Reveal Log File",
        synonyms: ["profiling", "diagnostics", "performance"],
        unavailable: info?.logPath ? undefined : "The log path is not known yet",
        run: () => void revealLog(),
      },
      {
        id: "copy-pr",
        title: "Copy Pull Request Link",
        unavailable: prUrl ? undefined : "No GitHub, GitLab or Bitbucket remote",
        run: () => {
          if (prUrl) void import("@tauri-apps/plugin-clipboard-manager").then((m) => m.writeText(prUrl));
        },
      },
      {
        id: "blame",
        title: "Blame This File",
        unavailable: diff.path ? undefined : "No file is open in the Diff panel",
        run: () => void showBlame(),
      },
      {
        id: "abort",
        title: "Abort Operation In Progress",
        unavailable: banner?.actions.includes("abort") ? undefined : "Nothing is in progress",
        run: () => void runBannerAction("abort"),
      },
      ...remoteCommands(
        {
          repository: repo !== null,
          remote: Boolean(network.primary),
          changes: worktree.total > 0,
          submodules: submoduleScope({
            children: submodules.children,
            open: submodules.open,
            selected: pickedFiles,
          }).choices.length,
          lfs: remoteOps.lfs,
          files: pickedFiles,
        },
        remoteActions,
      ),
    ];
  });

  async function openPullRequest() {
    if (!prUrl) return;
    if (tracked && tracked.ahead > 0) {
      const push = await ask(
        `${tracked.name} has ${tracked.ahead} commit(s) the remote has not seen. Push first?`,
        { title: "Create pull request", kind: "info" },
      );
      if (push) await runNetwork("push");
    }
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(prUrl);
  }

  async function runFind(text: string) {
    await finder.run(repository.current?.repo ?? null, text).catch((err) => errors.report(err, "Could not search"));
  }

  function pickFound(item: import("$lib/ipc").Found) {
    finderOpen = false;
    const id = repository.current?.repo;
    if (!id) return;
    if (item.kind === "commit") void commit.select(id, item.oid);
    if (item.kind === "branch") void switchTo({ name: item.label } as Branch);
    if (item.kind === "tag" && item.oid) void commit.select(id, item.oid);
    if (item.kind === "file" && commit.oid) openDiff(item.label);
  }

  function runCommand(command: PaletteCommand) {
    paletteOpen = false;
    recentCommands = [command.id, ...recentCommands.filter((id) => id !== command.id)].slice(0, 8);
    command.run();
  }

  // Accords that appear in the native menu are owned by it: handling them here too
  // would run the command twice for one keypress.
  /** True while the keystroke belongs to whatever the user is typing in. */
  function typing(event: KeyboardEvent): boolean {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return false;
    return (
      target.isContentEditable ||
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement
    );
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      settingsOpen = false;
      paletteOpen = false;
      finderOpen = false;
      return;
    }

    // F6 walks the panels. Ctrl+Tab is left to the window: the menu agent owns the
    // accelerators, and browsers and hosts both claim that pair (issue 15).
    if (event.key === "F6") {
      event.preventDefault();
      focused = step(focused, (panel) => layout.visible(panel), event.shiftKey ? -1 : 1);
      return;
    }

    // Select All belongs to the focused panel, and the graph declines it on purpose.
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a" && !typing(event)) {
      if (!allowsSelectAll(focused)) event.preventDefault();
    }
  }

  /** What Cancel goes back to. Taken when the dialog opens, not when it closes: by the
      time it closes, everything has already been applied and saved (R-122). */
  let settingsAtOpen = $state.raw<Settings | null>(null);
  let toolbarAtOpen: readonly string[] = [];
  /** The page Preferences opens on: right-click on the toolbar lands on Toolbar. */
  let settingsStart = $state<string | undefined>(undefined);

  function openSettings(page?: string) {
    settingsAtOpen = { ...settings.current };
    toolbarAtOpen = toolbar.layout;
    settingsStart = page;
    settingsOpen = true;
  }

  async function revertSettings() {
    const before = settingsAtOpen;
    settingsOpen = false;
    await toolbar.setLayout(toolbarAtOpen);
    if (before) await settings.apply(before);
  }

  async function applySettings(next: Settings, keymap: import("$lib/keymap").Keymap) {
    const before = settings.current;
    const touched = (Object.keys(next) as (keyof Settings)[]).filter(
      (key) => next[key] !== before[key],
    );
    await settings.apply(next);
    await settings.setKeymap(keymap);
    pushMenuState();

    const id = repository.current?.repo;
    if (!id || !diff.spec || !diff.path) return;
    if (touched.includes("ignoreWhitespace")) {
      await diff.setWhitespace(id, settings.current.ignoreWhitespace);
    } else if (touched.some((key) => REDIFF.includes(key))) {
      await diff.load(id, diff.spec, diff.path);
    }
  }

  // The watcher is the only way Cogit learns about work done in a terminal alongside it.
  // One `git commit` arrives as four events, so they are collected and answered once.
  let pending = new Set<import("$lib/ipc").ChangeKind>();
  let settling: ReturnType<typeof setTimeout> | undefined;
  const SETTLE_MS = 120;

  function onDiskChange(change: import("$lib/ipc").RepoChanged) {
    const id = repository.current?.repo;
    if (!id || id.valueOf() !== change.repo.valueOf()) return;
    if (change.kind !== "hooks") stale = markStale(stale, change.kind);
    pending.add(change.kind);
    clearTimeout(settling);
    settling = setTimeout(() => void applyDiskChanges(), SETTLE_MS);
  }

  async function applyDiskChanges() {
    const id = repository.current?.repo;
    const epoch = repository.epoch;
    const left = () => repository.epoch !== epoch;
    const plan = planFor(pending);
    pending = new Set();
    if (!id) return;

    // A hook edited outside Cogit is only interesting while the panel is open.
    if (plan.hooks && hooks.open) void hooks.refresh(id);
    if (!plan.cascade) return;

    if (plan.refs) {
      protection = new Map();
      await repository.refresh();
      if (left()) return;
    }
    stale = freshen(stale, ["repositories", "refs"]);

    if (plan.worktree && commit.oid === null) {
      await worktree.load(id);
      if (left()) return;
      stale = freshen(stale, ["files", "commit"]);
    }
    void worktrees.refresh(id);

    // Reads the status itself, which is why nothing above does it a second time.
    await afterMutation();
    if (left()) return;
    stale = freshen(stale, ["diff", "files", "commit"]);

    if (plan.refs) await graph.load(id, graph.query);
    stale = freshen(stale, ["graph", "refs"]);
  }

  function filterGraph(query: import("$lib/ipc").CommitQuery) {
    const id = repository.current?.repo;
    if (!id) return;
    commit.clear();
    diff.clear();
    void graph.load(id, query);
  }

  async function stage(paths: string[]) {
    await mutate((id) => worktree.stage(id, paths), paths);
  }

  async function unstage(paths: string[]) {
    await mutate((id) => worktree.unstage(id, paths), paths);
  }

  async function ignore(paths: string[]) {
    await mutate((id) => addToGitignore(id, paths), paths);
  }

  async function discard(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    const what = paths.length === 1 ? paths[0] : `${paths.length} files`;
    const confirmed = await confirmation.ask({
      title: "Discard",
      message: listedMessage(`Discard the changes in ${what}? Undo can bring them back.`, paths),
      confirm: "Discard",
      warning: true,
    });
    if (!confirmed) return;
    await mutate((repo) => worktree.discard(repo, paths), paths);
  }

  /** Toolbar Discard asks in the app's own modal, focus on Cancel (R-255). */
  async function discardFromToolbar(paths: string[]) {
    if (paths.length === 0) return;
    const what = paths.length === 1 ? paths[0] : `${paths.length} files`;
    const go = await confirmation.ask({
      title: "Discard Changes",
      message: `Discard the changes in ${what}? Undo can bring them back.`,
      confirm: "Discard",
      warning: true,
    });
    if (go) await mutate((repo) => worktree.discard(repo, paths), paths);
  }

  /** To the Recycle Bin, from the menu and the list's own Delete button alike (#40). */
  async function deleteFromDisk(paths: string[]) {
    const id = repository.current?.repo;
    if (!id || paths.length === 0) return;
    const what = paths.length === 1 ? paths[0] : `${paths.length} files`;
    const bin = (await desktop.load()).windowsShells ? "the Recycle Bin" : "the Trash";
    const confirmed = await confirmation.ask({
      title: "Delete",
      message: listedMessage(`Move ${what} to ${bin}?`, paths),
      confirm: "Delete",
      warning: true,
    });
    if (!confirmed) return;
    await mutate((repo) => fileMenus.moveToTrash(repo, paths), paths);
  }

  /** False when nothing was committed: the box keeps the message for another try. */
  async function commitStaged(message: string, amend: boolean, noVerify: boolean): Promise<boolean> {
    const id = repository.current?.repo;
    if (!id) return false;

    if (amend && (await isPublished(id, "HEAD").catch(() => false))) {
      const go = await ask(
        "This commit is already on a remote. Amending it gives it a new id, so the branch " +
          "will need a force-push and anyone who pulled it will have to reset. Continue?",
        { title: "Amend a published commit", kind: "warning" },
      );
      if (!go) return false;
    }

    // An empty list would commit every staged file, the hidden ones included.
    if (scope.empty) return false;
    if (scope.paths) {
      const listed = scope.paths.join("\n");
      const confirmed = await ask(
        `Commit only these ${scope.paths.length} file(s)?\n\n${listed}\n\n${scope.warning}.`,
        { title: "Commit what you see", kind: "warning" },
      );
      if (!confirmed) return false;
    }
    const epoch = repository.epoch;
    await worktree.commit(id, message, amend, noVerify, scope.paths ?? []);
    if (worktree.error) return false;
    if (repository.epoch !== epoch) return true;
    diff.clear();
    await repository.refresh();
    await afterMutation();
    void graph.load(id, graph.query);
    return true;
  }

  /** `worked` is the repository the change was made in; once the panels show another,
      reloading them would clear that one's selection and diff for nothing. */
  async function afterRefChange(worked?: RepoId) {
    const id = repository.current?.repo;
    if (!id || (worked !== undefined && worked !== id)) return;
    const epoch = repository.epoch;
    commit.clear();
    diff.clear();
    await repository.refresh();
    if (repository.epoch !== epoch) return;
    await worktree.load(id);
    if (repository.epoch !== epoch) return;
    await afterMutation();
    if (repository.epoch !== epoch) return;
    void graph.load(id, graph.query);
  }

  const refTreeBase = $derived({
    head: repo?.head,
    branches: repo?.branches ?? [],
    tags: repo?.tags ?? [],
    stashes: stashes.entries,
    lost: recovery.lost,
    remoteUrls: refs.urls,
    tagSeparator: repo?.tagGroupSeparator,
  });
  const refTreeInput = $derived({ ...refTreeBase, collapsed: refs.collapsed, filter: refFilter });

  // A heading that arrives after the open — stashes, lost commits — arrives folded (R-154).
  $effect(() => {
    const nodes = buildRefTree({ ...refTreeBase, collapsed: new Set(), filter: "" });
    untrack(() => refs.know(nodes));
  });

  /** Ticking a box changes which tips the walk starts from, so the graph is rebuilt.
      The ticks live on the graph store, so a later search keeps them. */
  async function reloadGraph() {
    const id = repo?.repo;
    if (!id) return;
    const watch = measure("reload-graph");
    const nodes = buildRefTree({ ...refTreeInput, filter: "", collapsed: new Set() });
    graph.visibleRefs = visibleTips(nodes, refs.visible);
    await graph.load(id, graph.query);
    watch.stop(`${graph.total} commits`);
  }

  /** A click on the text selects the ref and centres the graph on its tip. A stash is not
      a commit the user chose, so it takes over the Files panel instead (T5.2). */
  function selectRef(node: RefNode) {
    const id = repo?.repo;
    if (!id) return;

    if (node.kind === "stash") {
      commit.clear();
      diff.clear();
      void stashView.select(id, Number(node.id.slice("stash:".length)));
      return;
    }

    stashView.clear();
    if (!node.oid) return;
    void commit.select(id, node.oid);
    graph.requestReveal(node.oid);
  }

  /** One side of a stash part against the commit it was taken from. */
  function openStashDiff(part: "worktree" | "index" | "untracked", path: string) {
    const id = repo?.repo;
    const spec = stashView.spec(part);
    if (!id || !spec) return;
    void diff.load(id, spec, path);
  }

  function activateRef(node: RefNode) {
    if (node.kind === "stash") void applyStash(Number(node.id.slice("stash:".length)), false);
    else if (node.kind === "tag") {
      if (node.tag) void checkoutTag(node.tag);
    } else if (node.kind === "lost" && node.oid) {
      const found = recovery.lost.find((row) => row.oid === node.oid);
      if (found) void recoverCommit(found);
    }
  }

  async function switchTo(branch: Branch) {
    const id = repository.current?.repo;
    if (!id) return;

    const elsewhere = await worktrees.holding(id, branch.name);
    if (elsewhere && !elsewhere.missing) {
      const go = await ask(
        `${branch.name} is checked out in the worktree at ${elsewhere.path}. Switch to it?`,
        { title: "Branch is in another worktree", kind: "info" },
      );
      if (go) await activate(elsewhere.path);
      return;
    }

    try {
      await checkout(id, { kind: "branch", name: branch.name });
    } catch (err) {
      // Only git knows whether the working tree is really in the way: many dirty checkouts
      // are fine, so the offer is made after its refusal, not before every switch.
      if (!(await offerAutostash(err, branch))) errors.report(err, "Could not switch branches");
      return;
    }
    await afterRefChange(id);
  }

  /** Stash, switch, put the changes back — what `--autostash` does for rebase and pull. */
  async function offerAutostash(err: unknown, branch: Branch): Promise<boolean> {
    const id = repository.current?.repo;
    if (!id || !(err instanceof CogitError) || err.detail.kind !== "command") return false;
    const blocked = blockedByLocalChanges(err.detail.data.stderr);
    if (!blocked) return false;

    const what =
      blocked.length === 0
        ? "Local changes are in the way"
        : `${blocked.length} file(s) are in the way: ${blocked.slice(0, 5).join(", ")}`;
    const confirmed = await ask(
      `${what}. Stash them, switch to ${branch.name}, then put them back?`,
      { title: "Switch branch", kind: "warning" },
    );
    if (!confirmed) return false;

    try {
      await stashes.push(id, `cogit: autostash before switching to ${branch.name}`, true);
    } catch (failed) {
      errors.report(failed, "Could not stash the changes");
      await afterRefChange(id);
      return true;
    }
    try {
      await checkout(id, { kind: "branch", name: branch.name });
    } catch (failed) {
      // The changes are in the stash just made; left there, they would look lost.
      await stashes
        .apply(id, 0, true)
        .catch((err) => errors.report(err, "Your changes are in stash@{0}: they could not be put back"));
      errors.report(failed, "Could not switch branches");
      await afterRefChange(id);
      return true;
    }
    // Popping can conflict; the state banner then takes over, which is the honest outcome.
    await stashes.apply(id, 0, true).catch((failed) => errors.report(failed, "Could not put the changes back"));
    await afterRefChange(id);
    return true;
  }

  async function undo() {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await safety.undo(id);
    } catch (err) {
      errors.report(err, "Could not undo");
      return;
    }
    await afterRefChange(id);
  }

  async function showBlame() {
    const id = repository.current?.repo;
    const path = diff.path;
    if (!id || !path) return;
    await openBlame(id, path, commit.oid ?? "HEAD").catch((err) =>
      errors.report(err, "Could not open blame"),
    );
  }

  async function stageLines(selected: ReadonlySet<string>, reverse: boolean) {
    const path = diff.shownPath;
    if (!path) return;
    await mutate(() => diff.stageLines(selected, reverse), [path]);
  }

  async function refreshSubmodule(row: import("$lib/module-tree").ModuleRow) {
    const init = row.module.state === "notInitialised";
    // A nested module is updated by the repository that owns it, which is the submodule
    // above it in the tree, not the one at the top.
    const owner =
      row.parent === "" ? submodules.owner : ((await openedModule(row.parent))?.repo ?? null);
    if (!owner) return;
    await submodules.update(owner, row.path, init).catch((err) => errors.report(err, "Could not update the submodule"));
    await afterWorkingTreeChange();
  }

  /** Opens a submodule in the panels without listing it as a repository of its own
      (doc/12-risks.md, R-109). The tree keeps showing it where it is, and the tree is
      the one thing not forgotten, because it is what the click came from. */
  async function openModule(row: import("$lib/module-tree").ModuleRow) {
    // Nothing to open until it has been checked out; a double-click there means "get it".
    if (row.module.state === "notInitialised") {
      await refreshSubmodule(row);
      return;
    }
    const epoch = repository.epoch;
    const opened = await openedModule(row.key);
    // Clicked somewhere else meanwhile: taking the submodule now would overtake that.
    if (!opened || repository.epoch !== epoch) return;

    // Everything except the tree: it belongs to the repository in the list, and the click
    // came from it (doc/12-risks.md, R-129).
    forgetPanelsKeepingTheTree();
    worktrees.ownerRoot = null;
    submodules.open = row.key;
    repository.keep();
    repository.adopt(opened);
    refs.adopt(opened.root, buildRefTree({ ...refTreeInput, filter: "", collapsed: new Set() }));
    void reloadGraph();
    void refs.loadUrls(opened.repo);
    void worktrees.refresh(opened.repo);
    void flow.refresh(opened.repo);
    await afterMutation();
  }

  /** From the Diff panel, where a submodule that was never checked out says so. */
  async function initSubmoduleAt(path: string) {
    await mutate((id) => submodules.update(id, path, true));
  }

  /** The repository behind a node of the submodule tree, opened but not listed. The key
      is from the tree's owner, not from whichever submodule the panels show: that mix-up
      is what made every submodule after the first one fail (R-149). */
  async function openedModule(key: string) {
    const owner = submodules.owner;
    if (!owner) return null;
    try {
      return await openSubmodule(owner, key);
    } catch (err) {
      const detail = (err as { detail?: import("$lib/ipc").GitError }).detail;
      if (detail?.kind === "moduleUnavailable" && detail.data.reason === "notInitialised") {
        await offerInitialise(key);
        return null;
      }
      errors.report(err, "Could not open the submodule");
      return null;
    }
  }

  /** A warning's own button. Fetching is the one fix Cogit runs for the user: it changes
      nothing but the submodule's object database (R-179). */
  async function runNoticeAction(action: import("$lib/health").HealthAction) {
    const owner = submodules.owner;
    if (action.id !== FETCH_MODULES || !owner) return;
    for (const key of action.targets) {
      try {
        const opened = await openSubmodule(owner, key);
        const remote = await primaryRemote(opened.repo);
        if (remote) await fetchRemote(opened.repo, remote, () => {});
      } catch (err) {
        errors.report(err, `Could not fetch in ${key}`);
      }
    }
    await health.recheck();
    await submodules.refresh();
  }

  /** Never changes the repository unasked: the answer is a question (R-149). */
  async function offerInitialise(key: string) {
    const row = submodules.rows.find((entry) => entry.key === key);
    if (!row) return;
    const go = await ask(`Submodule ${key} is not initialised. Initialise and check it out now?`, {
      title: "Submodule is not initialised",
      kind: "info",
      okLabel: "Initialise",
      cancelLabel: "Cancel",
    });
    if (go) await refreshSubmodule(row);
  }

  async function recoverCommit(lost: import("$lib/ipc").CommitRow) {
    const id = repository.current?.repo;
    if (!id) return;
    const name = await prompt.ask({
      title: "Recover Commit",
      label: "Branch name for the recovered commit",
      value: "recovered",
      confirm: "Create Branch",
      validate: (value) => branchNameProblem(value, repository.localBranches.map((entry) => entry.name)),
    });
    if (!name) return;
    try {
      await createBranch(id, name, lost.oid, false);
    } catch (err) {
      errors.report(err, "Could not recover the commit");
      return;
    }
    await afterRefChange(id);
  }

  /** Toolbar Merge and Rebase act on the selection in Graph or Branches (task #31). */
  async function mergeSelected() {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (!id || !oid) return;
    try {
      await mergeInto(id, {
        source: refAt(oid, repo?.branches ?? []),
        noFastForward: false,
        squash: false,
        message: null,
      });
    } catch (err) {
      errors.report(err, "Could not merge");
    }
    await afterRefChange(id);
  }

  async function rebaseSelected() {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (!id || !oid) return;
    try {
      await rebaseOnto(id, { onto: refAt(oid, repo?.branches ?? []), autostash: true });
    } catch (err) {
      errors.report(err, "Could not rebase");
    }
    await afterRefChange(id);
  }

  async function runNetwork(kind: "fetch" | "pull" | "push") {
    const id = repository.current?.repo;
    const remote = network.primary;
    if (!id) return;
    if (!remote) {
      errors.message("This repository has no remote.", `Could not ${kind}`);
      return;
    }
    const epoch = repository.epoch;
    try {
      if (kind === "fetch") await network.fetch(id, remote);
      if (kind === "pull") await network.pull(id, remote, true);
      if (kind === "push") await network.push(id, remote, false);
    } catch (err) {
      errors.report(err, `Could not ${kind}`);
      if (repository.epoch === epoch) await afterMutation();
      return;
    }
    if (repository.epoch === epoch) await afterRefChange(id);
  }

  /** Pull as the toolbar's own choices say: which remotes to fetch first, whether to delete
      merged branches afterwards, and the fast-forward setting from Preferences (#26). Each
      step waits for the one before; the first failure stops the rest. */
  async function runRemoteSteps(steps: readonly ("pull" | "push")[], failure: string) {
    const id = repository.current?.repo;
    if (!id) return;
    const epoch = repository.epoch;
    try {
      const plan = remotePlan(steps, {
        remotes: network.remotes,
        pullRemote,
        pushRemote: network.primary,
        scope: toolbar.prefs.pullScope,
        ffOnly: settings.current.pullMode === "ffOnly",
        deleteMerged: toolbar.prefs.deleteMergedAfterPull,
      });
      for (const step of plan) {
        if (step.kind === "fetch") await network.fetch(id, step.remote);
        if (step.kind === "pull") await network.pull(id, step.remote, step.ffOnly);
        if (step.kind === "deleteMerged") await deleteMergedBranches(id);
        if (step.kind === "push") await network.push(id, step.remote, false);
      }
    } catch (err) {
      errors.report(err, failure);
      if (repository.epoch === epoch) await afterMutation();
      return;
    }
    await afterRefChange(id);
  }

  function pullNow() {
    return runRemoteSteps(["pull"], "Could not pull");
  }

  /** Sync ▸ an order: runs it, and the Sync button runs it from then on (#27). */
  function syncNow(order: SyncOrder = toolbar.prefs.syncOrder) {
    if (order !== toolbar.prefs.syncOrder) void toolbar.set("syncOrder", order);
    return runRemoteSteps(syncSteps(order), "Could not sync");
  }

  /** One failure does not stop the other remotes. */
  async function fetchRemotes(names: readonly string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    for (const remote of names) {
      await network.fetch(id, remote).catch((err) => errors.report(err, `Could not fetch ${remote}`));
    }
    await afterRefChange(id);
  }

  async function checkoutTag(tag: Tag) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await checkout(id, { kind: "commit", oid: tag.oid });
    } catch (err) {
      errors.report(err, "Could not check out the tag");
      return;
    }
    await afterRefChange(id);
  }

  /** The Stash dialog: a name and Stash All, + Keep Index or + Keep Working Tree (#29). */
  async function stashAll() {
    const id = repository.current?.repo;
    if (!id) return;
    const choice = await stashDialog.create();
    if (choice === null) return;
    await stashes
      .save(id, choice)
      .then(() => afterRefChange(id))
      .catch((err) => errors.report(err, "Could not stash"));
  }

  /** Everything, no dialog, Git's own message. */
  async function quickStashAll() {
    const id = repository.current?.repo;
    if (!id) return;
    await stashes
      .save(id, { mode: "all", message: "" })
      .then(() => afterRefChange(id))
      .catch((err) => errors.report(err, "Could not stash"));
  }

  /** Only these files; everything else stays in the working tree (T5.3). With `confirm`,
      the list is shown first. The toolbar and the file menu both come here (#29). */
  async function stashPaths(paths: string[], confirm = true) {
    const id = repository.current?.repo;
    if (!id || paths.length === 0) return;
    const message = confirm ? await stashDialog.selection(paths) : "";
    if (message === null) return;
    await stashSelection(id, paths, message)
      .then(() => afterRefChange(id))
      .catch((err) => errors.report(err, "Could not stash"));
  }

  function stashSelected(confirm = true) {
    return stashPaths(targetsOf("stash-selection", toolbarFacts), confirm);
  }

  async function applyStash(index: number, pop: boolean) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await stashes.apply(id, index, pop);
    } catch (err) {
      errors.report(err, "Could not apply the stash");
      return;
    }
    await afterRefChange(id);
  }

  /** Toolbar Apply Stash (#30). A conflicted apply still changed the working tree, so the
      panels reload either way; Git's output reaches the notification window. */
  async function applyNewestStash() {
    const id = repository.current?.repo;
    if (!id) return;
    await stashes
      .apply(id, 0, false)
      .catch((err) => errors.report(err, "Could not apply stash@{0}"));
    await afterRefChange(id);
  }

  async function runBannerAction(action: BannerAction) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      if (action === "abort") await abortOperation(id);
      if (action === "continue") await continueOperation(id);
      if (action === "skip") await skipOperation(id);
      if (action === "createBranch") {
        const name = await prompt.ask({
          title: "Create Branch",
          label: "Name for the new branch at this commit",
          confirm: "Create Branch",
          validate: (value) => branchNameProblem(value, repository.localBranches.map((entry) => entry.name)),
        });
        if (!name) return;
        await createBranch(id, name, null, true);
      }
    } catch (err) {
      errors.report(err, action === "createBranch" ? "Could not create the branch" : `Could not ${action}`);
      return;
    }
    await afterRefChange(id);
  }

  /** Double-click opens the file in its own window, which survives a webview reload. */
  function openInWindow(path: string, spec = diff.spec) {
    const id = repository.current?.repo;
    if (!id || !spec) return;
    void openCompareWindow(compareUrl(id, path, spec), `${path} — Cogit`).catch((err) =>
      errors.report(err, "Could not open the file window"),
    );
  }

  function openDiff(path: string) {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (!id || !oid) return;
    const spec = { kind: "commitVsParent", oid } as const;
    if (diff.shows(spec, path)) return;
    void diff.load(id, spec, path);
  }

  function openStagedDiff(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    if (diff.shows({ kind: "indexVsHead" }, path)) return;
    void diff.load(id, { kind: "indexVsHead" }, path);
  }

  function openWorktreeDiff(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    // A conflicted file has three sides; a two-sided diff of it says nothing useful.
    if (conflicts.paths.includes(path)) {
      diff.clear();
      void conflicts.open(id, path);
      return;
    }
    // A second click is not a toggle: it would fight the double-click that opens a window (#7).
    if (diff.shows({ kind: "workTreeVsIndex" }, path)) return;
    conflicts.close();
    const watch = measure("open-diff");
    void diff.load(id, { kind: "workTreeVsIndex" }, path).then(() => watch.stop(path));
  }

  /** Clearing everything the old repository put on screen — except the submodule tree,
      which belongs to the repository in the list and outlives the panels (R-129). */
  function forgetPanelsKeepingTheTree(keepWorktrees = false) {
    commit.clear();
    diff.clear();
    worktree.clear();
    stashes.clear();
    network.clear();
    recovery.clear();
    conflicts.clear();
    stashView.clear();
    if (!keepWorktrees) worktrees.clear();
    refs.clear();
  }

  function forgetPanels() {
    forgetPanelsKeepingTheTree();
    submodules.clear();
  }

  /** The root of the repository now on screen, or null when this open lost to a newer
      one or failed. */
  async function activate(root: string, restoreOid: string | null = null): Promise<string | null> {
    const story = `open:${root}`;
    trace(story, "activate: panels cleared");
    forgetPanels();
    worktrees.ownerRoot = null;
    const watch = measure("open-repository");
    if (!(await repository.open(root))) {
      trace(story, "activate: overtaken by a newer open, leaving the panels to it");
      return null;
    }
    const opened = repository.current;
    trace(story, `activate: repository.current is ${opened ? opened.name : "null"}`);
    if (opened) {
      refs.adopt(opened.root, buildRefTree({ ...refTreeInput, filter: "", collapsed: new Set() }));
      void submodules.own(opened.repo, opened.root);
      void health.check(opened.repo, opened.root, opened.name);
      session.setActive(opened.root);
      session.opened(opened.root);
      repoList.opened(opened.root);
      if (restoreOid) void commit.select(opened.repo, restoreOid);
      void timed(story, "graph", () => reloadGraph());
      void refs.loadUrls(opened.repo);
      void worktrees.refresh(opened.repo);
      void flow.refresh(opened.repo);
      await timed(story, "repository list", () => repository.refreshList());
      await timed(story, "everything else", () => afterMutation());
      trace(story, "activate: done");
    } else {
      trace(story, "activate: nothing opened, graph cleared");
      graph.clear();
    }
    watch.stop(`${repository.current?.branches.length ?? 0} refs`);
    // A failed open leaves the previous repository on screen.
    return repository.error ? null : (repository.current?.root ?? null);
  }

  /** A click in the Repositories list (#50): the one on screen reloads nothing, the owner
      of the submodule or worktree on screen comes back from what was kept, keeping its
      submodule tree, and only another repository goes through a full open. */
  async function selectRepository(entry: import("$lib/ipc").RepoOverview) {
    const phase = repository.phase;
    const step = repoClick(entry, {
      shown: repository.current?.repo ?? null,
      opening: phase.kind === "opening" ? phase.root : null,
      moduleOwner: submodules.open !== null ? submodules.owner : null,
      worktreeOwner: worktrees.ownerRoot,
    });
    trace(`open:${entry.root}`, `repository click: ${step}`);
    if (step === "open") await activate(entry.root);
    else if (step === "return") await comeBack(entry.root);
  }

  async function comeBack(root: string) {
    forgetPanelsKeepingTheTree();
    worktrees.ownerRoot = null;
    submodules.open = null;
    await repository.comeBack(root);
    const opened = repository.current;
    if (!opened) return;
    refs.adopt(opened.root, buildRefTree({ ...refTreeInput, filter: "", collapsed: new Set() }));
    void reloadGraph();
    void refs.loadUrls(opened.repo);
    void worktrees.refresh(opened.repo);
    void flow.refresh(opened.repo);
    await afterMutation();
  }

  /** Restores a past version into the working tree; HEAD stays where it is. */
  async function rollbackFiles(paths: string[]) {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    const what = paths.length > 0 ? paths.join(", ") : "every file";
    const go = await ask(`Restore ${what} as it was in ${shortOid(rev)}?`, {
      title: "Roll back",
      kind: "warning",
    });
    if (!go) return;
    await mutate((repo) => rollbackTo(repo, rev, paths), paths);
  }

  /** A drop never acts on its own: the user picks from the menu it opens. */
  function onBranchDrop(sourceName: string, target: Branch) {
    const source = repository.localBranches.find((b) => b.name === sourceName);
    if (!source) return;
    const canFastForward = target.isHead && source.oid !== target.oid;
    const payload = { kind: "branch" as const, id: sourceName };
    const onto = { kind: "branch" as const, id: target.name };
    const head = repository.localBranches.find((b) => b.isHead)?.name ?? null;
    const actions = dropActions(payload, onto, canFastForward, head);
    if (actions.length === 0) return;
    dropMenu = { actions, source: payload, target: onto, x: pointer.x, y: pointer.y };
  }

  async function runDropAction(action: DropAction) {
    const menu = dropMenu;
    dropMenu = null;
    const id = repository.current?.repo;
    if (!menu || !id || action.disabled) return;

    if (menu.source.kind === "commit") {
      await replan(action, menu.source.id, menu.target.id);
      return;
    }

    if (action.destructive) {
      const go = await ask(`${action.title}. This rewrites history. Continue?`, {
        title: "Rewrite history",
        kind: "warning",
      });
      if (!go) return;
    }

    try {
      if (action.id === "merge") {
        await mergeInto(id, {
          source: menu.source.id,
          noFastForward: false,
          squash: false,
          message: null,
        });
      } else if (action.id === "rebase") {
        await rebaseOnto(id, { onto: menu.target.id, autostash: true });
      } else if (action.id === "fastForward") {
        await mergeInto(id, {
          source: menu.source.id,
          noFastForward: false,
          squash: false,
          message: null,
        });
      }
    } catch (err) {
      errors.report(err, `Could not ${action.id === "rebase" ? "rebase" : "merge"}`);
    }
    await afterRefChange(id);
  }

  function onCommitDrop(sourceOid: string, targetOid: string) {
    const source = { kind: "commit" as const, id: sourceOid };
    const target = { kind: "commit" as const, id: targetOid };
    const actions = dropActions(source, target, false);
    if (actions.length === 0) return;
    dropMenu = { actions, source, target, x: pointer.x, y: pointer.y };
  }

  /** Squash and reorder are both one interactive rebase with a two-line plan. */
  async function replan(action: DropAction, source: string, target: string) {
    const id = repository.current?.repo;
    if (!id) return;
    const base = await mergeBaseOf(id, source, target);
    if (base === null) return;

    let plan;
    try {
      plan = await rebaseTodo(id, base);
    } catch (err) {
      errors.report(err, "Could not plan the rebase");
      return;
    }

    const from = plan.findIndex((entry) => entry.oid === source);
    const onto = plan.findIndex((entry) => entry.oid === target);
    if (from < 0 || onto < 0) {
      errors.message("Both commits have to be on the current branch above their common parent.", "Could not move the commit");
      return;
    }

    let moved = moveEntry(plan, from, onto < from ? onto + 1 : onto);
    if (action.id === "squash") {
      moved = moved.map((entry) =>
        entry.oid === source ? { ...entry, action: "squash" as const } : entry,
      );
    }

    rebaseBase = base;
    rebasePlan = moved;
    splitPublished = await isPublished(id, base).catch(() => false);
    rebaseOpen = true;
  }

  /** The parent of the older of the two, so the plan covers both. */
  async function mergeBaseOf(id: RepoId, a: string, b: string): Promise<string | null> {
    try {
      const plan = await rebaseTodo(id, `${a}^`);
      if (plan.some((entry) => entry.oid === b)) return `${a}^`;
      return `${b}^`;
    } catch {
      errors.message("Both commits have to be on the current branch.", "Could not move the commit");
      return null;
    }
  }

  /** Opens the plan editor for everything after the selected commit. */
  async function openRebase() {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    try {
      rebasePlan = await rebaseTodo(id, rev);
    } catch (err) {
      errors.report(err, "Could not start the interactive rebase");
      return;
    }
    if (rebasePlan.length === 0) {
      errors.message("This commit is the tip; there is nothing after it to rebase.", "Could not start the interactive rebase");
      return;
    }
    rebaseBase = rev;
    splitPublished = await isPublished(id, rev).catch(() => false);
    rebaseOpen = true;
  }

  async function runRebase() {
    const id = repository.current?.repo;
    if (!id) return;
    rebaseBusy = true;
    try {
      await interactiveRebase(id, rebaseBase, rebasePlan, rebasePaused);
      rebaseOpen = false;
      commit.clear();
    } catch (err) {
      errors.report(err, "Could not rebase");
    } finally {
      rebaseBusy = false;
    }
    await afterRefChange(id);
  }

  /** A real OS menu, not an HTML popup: the chosen id comes back as a menu command. */
  async function commitContext(oid: string, x: number, y: number) {
    const id = repository.current?.repo;
    if (!id) return;
    void learnProtection(oid);
    await refActions?.commitContext(oid, x, y);
  }

  /** The chosen item comes back through the same `menu-command` event as the menu bar,
      so the node it was opened on has to be remembered until then. */
  let refTarget = $state.raw<RefNode | null>(null);
  let refActions = $state<ReturnType<typeof RefActions>>();
  let fileTarget = $state.raw<FileScope | null>(null);
  let fileSection = $state<"worktree" | "index" | "commit">("worktree");
  let aboutOpen = $state(false);
  let configEdit = $state.raw<{
    scope: import("$lib/ipc").ConfigScope;
    file: import("$lib/ipc").ConfigFile;
    problem: { line: number | null; message: string } | null;
    saving: boolean;
  } | null>(null);

  async function openConfig(scope: import("$lib/ipc").ConfigScope) {
    const id = scope === "repository" ? (repository.current?.repo ?? null) : null;
    if (scope === "repository" && id === null) return;
    try {
      configEdit = { scope, file: await readGitConfig(id, scope), problem: null, saving: false };
    } catch (err) {
      errors.report(err, "Could not read the Git config");
    }
  }

  /** Git reads the text back before it is written; a refusal keeps the dialog open on
      the line it named. The config changes remotes and core.*, so the rest is re-read. */
  async function saveConfig(text: string) {
    const edit = configEdit;
    if (!edit) return;
    const id = edit.scope === "repository" ? (repository.current?.repo ?? null) : null;
    configEdit = { ...edit, saving: true };
    try {
      await writeGitConfig(id, edit.scope, text, edit.file.crlf);
    } catch (err) {
      const detail = (err as { detail?: import("$lib/ipc").GitError }).detail;
      if (detail?.kind === "configInvalid") {
        configEdit = { ...edit, saving: false, problem: detail.data };
        return;
      }
      configEdit = { ...edit, saving: false };
      errors.report(err, "Could not save the Git config");
      return;
    }
    configEdit = null;
    if (repository.current) {
      await repository.refresh();
      await afterMutation();
    }
  }

  /** Right-clicking a ticked row acts on the whole tick; right-clicking any other row
      acts on that one, which is what every file manager does. */
  function fileScope(path: string): string[] {
    return markedFiles.includes(path) ? [...markedFiles] : [path];
  }

  /** `section` is the list the row sits in: "Staged" is the index, the rest the working
      tree (#40). A commit's files get their own menu (#41). */
  async function fileContext(path: string, event: MouseEvent, section?: string) {
    const id = repository.current?.repo;
    if (!id) return;
    const { clientX: x, clientY: y } = event;
    const paths = fileScope(path);
    const info = await desktop.load();

    if (!onWorkingTree) {
      const statuses = paths.map((each) => commit.files.find((file) => file.path === each)?.status ?? "modified");
      const clicked = commit.files.find((file) => file.path === path);
      const present = await fileMenus.presentOnDisk(id, [path]).catch(() => [] as string[]);
      fileTarget = { path, paths, statuses, rev: commit.oid, oldPath: clicked?.oldPath ?? null };
      fileSection = "commit";
      const items = commitFileMenu({
        status: clicked?.status ?? "modified",
        count: paths.length,
        onDisk: present.includes(path),
        fileManager: info.fileManager,
      });
      await popupContextMenu(items, x, y).catch(() => {});
      return;
    }

    const index = section === "Staged";
    const own = index ? worktree.staged : worktree.unstaged;
    const every = [...own, ...worktree.staged, ...worktree.unstaged];
    const statuses = paths.map((each) => every.find((file) => file.path === each)?.status ?? "modified");
    fileTarget = { path, paths, statuses, rev: null, oldPath: null };
    fileSection = index ? "index" : "worktree";
    const items = worktreeFileMenu({
      section: index ? "index" : "worktree",
      statuses,
      staged: paths.some((each) => worktree.staged.some((file) => file.path === each)),
      unstaged: paths.some((each) => worktree.unstaged.some((file) => file.path === each)),
      fileManager: info.fileManager,
    });
    await popupContextMenu(items, x, y).catch(() => {});
  }

  function runFileCommand(id: string): boolean {
    const scope = fileTarget;
    const root = repository.current?.root;
    if (scope === null || !root || !id.startsWith("file-")) return false;
    return runFileMenuCommand(id, scope, fileActions, { root, separator: desktop.info.separator });
  }

  /** Where each file menu command lands; most are the panel's own actions. */
  const fileActions: FileActions = {
    openFile: (path) => void onDesktop((root) => fileMenus.openOnDesktop(`${root}/${path}`), "Could not open the file"),
    openVersion: (path, rev) => void withRepo((id) => fileMenus.openReadOnly(id, rev, path), "Could not open the file"),
    reveal: (path) => void onDesktop((root) => fileMenus.revealOnDesktop(`${root}/${path}`), "Could not reveal the file"),
    showChanges: (path) => openInWindow(path, fileSpec(path)),
    compareWithWorkTree: (path, rev) => openInWindow(path, { kind: "commitVsWorkTree", oid: rev }),
    log: (path) => filterGraph({ ...graph.query, path }),
    blame: (path) => void blameOne(path),
    investigate: (path) => investigateFile(path),
    commit: (paths) => void commitFiles(paths),
    stash: (paths) => void stashPaths(paths),
    stage: (paths) => void stage(paths),
    unstage: (paths) => void unstage(paths),
    indexEditor: (path) => void openIndexEditor(path),
    move: (path) => void askMove(path),
    resolve: (paths, side) => void mutate(async (id) => {
      for (const path of paths) await resolveConflict(id, path, side);
    }, paths),
    ignore: (paths) => void ignore(paths),
    discard: (paths) => void discard(paths),
    remove: (paths) => (removingFiles = paths),
    trash: (paths) => void deleteFromDisk(paths),
    saveAs: (path, rev) => void saveVersionAs(path, rev),
    applyChange: (path, oldPath, rev, reverse) => void applyFileChange(path, oldPath, rev, reverse),
    copy: (text) => void copyText(text),
    setFlag: (paths, flag, on) => void mutate((id) => fileMenus.setIndexFlag(id, paths, flag, on), paths),
  };

  /** What a double-click on that row shows: the side of the diff its list stands for. */
  function fileSpec(path: string): import("$lib/ipc").DiffSpec | null {
    if (fileSection === "commit") return commit.oid ? { kind: "commitVsParent", oid: commit.oid } : null;
    const staged = worktree.staged.some((file) => file.path === path);
    return fileSection === "index" && staged ? { kind: "indexVsHead" } : { kind: "workTreeVsIndex" };
  }

  async function withRepo(step: (id: import("$lib/ipc").RepoId) => Promise<unknown>, failure: string) {
    const id = repository.current?.repo;
    if (!id) return;
    await step(id).catch((err) => errors.report(err, failure));
  }

  async function onDesktop(step: (root: string) => Promise<unknown>, failure: string) {
    const root = repository.current?.root;
    if (!root) return;
    await step(root).catch((err) => errors.report(err, failure));
  }

  /** A working-tree file is investigated as it is on disk, a file of a commit at that commit. */
  function investigateFile(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    void openInvestigate(id, path, commit.oid ?? null).catch((err) =>
      errors.report(err, "Could not open Investigate"),
    );
  }

  /** Commit… from a file row: the files go into the index and the message box takes the
      cursor; the Commit panel commits what is staged, as it always does. */
  async function commitFiles(paths: string[]) {
    const pending = paths.filter((path) => worktree.unstaged.some((file) => file.path === path));
    if (pending.length > 0) await stage(pending);
    if (!layout.visible("commit")) layout.togglePanel("commit");
    await tick();
    document.querySelector<HTMLTextAreaElement>('textarea[aria-label="Commit message"]')?.focus();
  }

  let indexEditing = $state.raw<{
    path: string;
    sides: import("$lib/ipc/file-menus").IndexEditorSides;
    saving: boolean;
  } | null>(null);

  async function openIndexEditor(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      indexEditing = { path, sides: await fileMenus.indexEditorSides(id, path), saving: false };
    } catch (err) {
      errors.report(err, "Could not read the file for the Index Editor");
    }
  }

  async function saveIndexEditor(edited: { index: string | null; worktree: string | null }) {
    const open = indexEditing;
    if (!open) return;
    indexEditing = { ...open, saving: true };
    const saved = await mutate(
      (id) => fileMenus.writeIndexEditor(id, open.path, edited.index, edited.worktree),
      [open.path],
    );
    indexEditing = saved ? null : { ...open, saving: false };
  }

  async function askMove(path: string) {
    const target = await prompt.ask({
      title: "Move or Rename",
      label: "New path, relative to the repository",
      value: path.replace(/\/+$/, ""),
      confirm: "Move",
      validate: (value) => {
        const next = value.trim().replace(/\\/g, "/");
        if (next === "") return "Enter a path.";
        if (next.startsWith("/") || /^[A-Za-z]:/.test(next) || next.split("/").includes("..")) {
          return "Stay inside the repository.";
        }
        return next === path.replace(/\/+$/, "") ? "That is where it is now." : null;
      },
    });
    if (target === null) return;
    const to = target.trim().replace(/\\/g, "/");
    await mutate((id) => fileMenus.movePath(id, path, to), [path, to]);
  }

  let removingFiles = $state.raw<string[] | null>(null);

  // Everything here was asked or opened about the repository on screen. Answered after
  // the panels moved on (a folder dropped on the window, Ctrl+O), it acted on the next
  // one: Discard in B, A's config saved over B's.
  $effect(() =>
    repository.onLeave(() => {
      leaveRepositoryDialogs();
      stale = new Set();
      // The Commit box seeds an empty draft from it, under the next repository's key.
      template = null;
      if (configEdit?.scope === "repository") configEdit = null;
      indexEditing = null;
      removingFiles = null;
      dropMenu = null;
      worktreeRemoval = null;
      addWorktreeOpen = false;
      repoSettingsOpen = false;
      rebaseOpen = false;
      splitOpen = false;
    }),
  );

  async function removeFiles(paths: string[], deleteLocal: boolean) {
    removingFiles = null;
    await mutate((id) => fileMenus.removeFromRepository(id, paths, deleteLocal), paths);
  }

  async function saveVersionAs(path: string, rev: string) {
    const id = repository.current?.repo;
    if (!id) return;
    const { save } = await import("@tauri-apps/plugin-dialog");
    const target = await save({ title: `Save ${fileName(path)} from ${shortOid(rev)}`, defaultPath: fileName(path) });
    if (!target) return;
    await fileMenus.saveBlob(id, rev, path, target).catch((err) => errors.report(err, "Could not save the file"));
  }

  /** Cherry-Pick and Revert of one file's change: a patch applied with a three-way
      fallback, so the result is in the working tree and the index to review. */
  async function applyFileChange(path: string, oldPath: string | null, rev: string, reverse: boolean) {
    await mutate((id) => fileMenus.applyCommitFile(id, rev, path, oldPath, reverse), [path]);
  }

  /** Blame of any row in Files, in the Blame window like every other entry point. */
  async function blameOne(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    await openBlame(id, path, commit.oid ?? "HEAD").catch((err) =>
      errors.report(err, "Could not open blame"),
    );
  }

  async function refContext(node: RefNode, x: number, y: number) {
    if (refActions?.claims(node)) {
      await refActions.branchesContext(node, x, y);
      return;
    }
    const items = refMenu({ kind: node.kind });
    if (items.length === 0) return;
    refTarget = node;
    await popupContextMenu(items, x, y).catch(() => {});
  }


  /** One failure must not stop the rest: the point of Fetch All is not doing it by hand. */
  async function fetchAll() {
    const roots = markedRepos.length > 0 ? markedRepos : repository.openRepos.map((e) => e.root);
    const targets = repository.openRepos.filter((entry) => roots.includes(entry.root));
    if (targets.length === 0) return;

    const watch = measure("fetch-all");
    let failed = 0;
    bulk = { label: "Fetching", done: 0, total: targets.length };

    for (const [index, entry] of targets.entries()) {
      try {
        const remote = await primaryRemote(entry.repo);
        if (remote) await fetchRemote(entry.repo, remote, () => {});
      } catch (err) {
        failed += 1;
        errors.report(err, "Could not fetch");
      }
      bulk = { label: "Fetching", done: index + 1, total: targets.length, failed };
    }

    bulk = undefined;
    watch.stop(`${targets.length} repositories, ${failed} failed`);
    await repository.refreshList();
    await afterRefChange();
  }

  /** The remote a bulk fetch should use: the tracked one, else the only one there is. */
  async function primaryRemote(id: import("$lib/ipc").RepoId): Promise<string | null> {
    const names = await listRemotes(id).catch(() => [] as string[]);
    return names.includes("origin") ? "origin" : (names[0] ?? null);
  }

  /** The verdict is information. Nothing is aborted, continued or skipped on its account. */
  async function runPauseCheck() {
    const id = repo?.repo;
    if (!id || checkCommand.trim() === "") return;
    checking = true;
    try {
      checkVerdict = await runCheck(id, checkCommand);
    } catch (err) {
      checkVerdict = null;
      errors.report(err, "Could not run the check");
    } finally {
      checking = false;
      await output.refresh();
      await output.refreshProblems();
    }
  }

  /** Every flow step ends the same way: refresh what the panels show, or report why not. */
  async function runFlow(step: () => Promise<void>) {
    try {
      await step();
      await repository.refresh();
      await afterMutation();
      await reloadGraph();
    } catch (err) {
      errors.report(err, "Could not run the git-flow step");
    }
  }

  async function askFlowStart(kind: import("$lib/ipc").FlowKind) {
    const id = repository.current?.repo;
    if (!id) return;
    const name = await prompt.ask({
      title: `Start a ${kind}`,
      label: "Name",
      confirm: "Start",
      validate: (value) => branchNameProblem(value, repository.localBranches.map((entry) => entry.name)),
    });
    if (name === null) return;
    await runFlow(() => flow.start(id, kind, name));
  }

  async function askFlowFinish() {
    const id = repository.current?.repo;
    const branch = flow.current;
    if (!id || !branch) return;
    if (branch.kind === "feature") {
      void runFlow(() => flow.finish(id, branch.kind, branch.name, null));
      return;
    }
    const tag = await prompt.ask({
      title: `Finish ${branch.full}`,
      label: "Tag for the release, or empty for none",
      value: branch.name,
      confirm: "Finish",
      validate: optional,
    });
    if (tag === null) return;
    await runFlow(() => flow.finish(id, branch.kind, branch.name, tag.trim() || null));
  }

  async function askAddGroup() {
    const name = await prompt.ask({ title: "Add a group", label: "Name", confirm: "Add", validate: textProblem });
    if (name !== null) repoGroups.add(name);
  }

  async function groupContext(id: string, x: number, y: number) {
    groupTarget = id;
    await popupContextMenu(
      [
        { id: "group-open", label: "Open Repository here…", enabled: true, separator: false },
        { id: "", label: "", enabled: false, separator: true },
        { id: "group-rename", label: "Rename this group…", enabled: true, separator: false },
        { id: "group-remove", label: "Delete this group", enabled: true, separator: false },
      ],
      x,
      y,
    ).catch(() => {});
  }

  /** Returns true when the id belonged to a group heading and was handled here. */
  function runGroupCommand(id: string): boolean {
    const target = groupTarget;
    if (target === null) return false;

    if (id === "group-open") {
      // Opened and filed in one step, so a fresh group is not a dead end (issue 7).
      // Cancelled, the repository already on screen is not the one to file there.
      void pickRepository().then((root) => {
        if (root) repoGroups.assign(root, target);
      });
      return true;
    }
    if (id === "group-rename") {
      void prompt
        .ask({
          title: "Rename group",
          label: "Name",
          value: repoGroups.groups.names[target] ?? "",
          confirm: "Rename",
          validate: textProblem,
        })
        .then((name) => {
          if (name !== null) repoGroups.rename(target, name);
        });
      return true;
    }
    if (id === "group-remove") {
      // The repositories go back to the ungrouped bucket, so nothing is lost by deleting.
      repoGroups.remove(target);
      return true;
    }
    return false;
  }

  /** The dialog lists what is uncommitted and asks separately for --force (R-184). */
  async function removeWorktreeAt(entry: import("$lib/ipc").WorktreeEntry) {
    worktreeRemoval = { entry, changes: null };
    const changes = await worktrees.changes(entry.path).catch((err) => {
      errors.report(err, "Could not read the worktree's changes");
      return null;
    });
    if (changes === null) {
      worktreeRemoval = null;
      return;
    }
    if (worktreeRemoval?.entry.path === entry.path) worktreeRemoval = { entry, changes };
  }

  async function confirmWorktreeRemoval(force: boolean) {
    const target = worktreeRemoval?.entry;
    worktreeRemoval = null;
    if (!target) return;
    await worktrees
      .remove(target.path, force)
      .catch((err) => errors.report(err, "Could not remove the worktree"));
    await safety.refresh();
  }

  async function pruneWorktreesHere() {
    const stale = worktrees.entries.filter((entry) => entry.missing);
    if (stale.length === 0) return;
    const names = stale.map((entry) => entry.name).join(", ");
    const confirmed = await ask(
      `Forget ${stale.length === 1 ? "the missing worktree" : `${stale.length} missing worktrees`} (${names})? ` +
        "Only Git's registration is removed; nothing on disk is touched. Locked ones are kept.",
      { title: "Prune Obsolete Worktrees", kind: "info", okLabel: "Prune", cancelLabel: "Cancel" },
    );
    if (!confirmed) return;
    await worktrees.prune().catch((err) => errors.report(err, "Could not prune worktrees"));
  }

  async function pruneWorktreeAt(entry: import("$lib/ipc").WorktreeEntry) {
    const confirmed = await ask(
      `Forget the worktree ${entry.name} at ${entry.path}? ` +
        "Only Git's registration of it is removed; nothing on disk is touched.",
      { title: "Prune Worktree", kind: "info", okLabel: "Prune", cancelLabel: "Cancel" },
    );
    if (!confirmed) return;
    await worktrees
      .pruneOne(entry.path)
      .catch((err) => errors.report(err, "Could not prune the worktree"));
  }

  /** Repair cannot guess where a folder went: the new place is asked for first. */
  async function repairWorktreeAt(entry: import("$lib/ipc").WorktreeEntry) {
    const picked = await openFolderDialog({
      directory: true,
      title: `Locate the folder of worktree ${entry.name}`,
    });
    if (typeof picked !== "string") return;
    await worktrees
      .repair(picked.replace(/\\/g, "/"))
      .catch((err) => errors.report(err, "Could not repair the worktree"));
  }

  async function addWorktreeFrom(request: {
    folder: string;
    branch: string;
    create: boolean;
    base: string | null;
  }) {
    addWorktreeOpen = false;
    await worktrees
      .add(request.folder.replace(/\\/g, "/"), request.branch, request.create, request.base)
      .catch((err) => errors.report(err, "Could not add the worktree"));
  }

  /** Like a submodule: the panels switch to it, the Repositories tree stays (R-184). */
  async function openWorktreeRow(entry: import("$lib/ipc").WorktreeEntry) {
    const owner = worktrees.repo;
    const listed = repository.current?.root ?? null;
    if (!owner || entry.missing || entry.isCurrent) return;
    const epoch = repository.epoch;
    let opened;
    try {
      opened = await openWorktree(owner, entry.path);
    } catch (err) {
      errors.report(err, "Could not open the worktree");
      return;
    }
    if (repository.epoch !== epoch) return;
    const ownerRoot = worktrees.ownerRoot ?? listed;
    const same = (a: string, b: string | null) =>
      b !== null && a.replaceAll("\\", "/") === b.replaceAll("\\", "/");
    forgetPanelsKeepingTheTree(true);
    worktrees.ownerRoot = same(opened.root, ownerRoot) ? null : ownerRoot;
    worktrees.selected = entry.path;
    repository.keep();
    repository.adopt(opened);
    refs.adopt(opened.root, buildRefTree({ ...refTreeInput, filter: "", collapsed: new Set() }));
    void reloadGraph();
    void refs.loadUrls(opened.repo);
    void worktrees.refresh(opened.repo);
    void flow.refresh(opened.repo);
    await afterMutation();
  }

  async function worktreeContext(entry: import("$lib/ipc").WorktreeEntry, x: number, y: number) {
    const chosen = await popupContextMenu(
      entry.missing
        ? [
            { id: "prune", label: "Prune", enabled: entry.locked === null },
            { id: "repair", label: "Repair…", enabled: true },
            { id: "", label: "", enabled: false, separator: true },
            { id: "copy", label: "Copy Path", enabled: true },
          ]
        : [
            { id: "open", label: "Open", enabled: !entry.isCurrent },
            { id: "reveal", label: "Reveal in File Manager", enabled: true },
            { id: "copy", label: "Copy Path", enabled: true },
            { id: "", label: "", enabled: false, separator: true },
            entry.locked === null
              ? { id: "lock", label: "Lock", enabled: !entry.isMain }
              : { id: "unlock", label: "Unlock", enabled: true },
            { id: "remove", label: "Remove…", enabled: removable(entry) },
          ],
      x,
      y,
    ).catch(() => null);

    if (chosen === "open") await openWorktreeRow(entry);
    if (chosen === "copy") await copyText(entry.path);
    if (chosen === "remove") await removeWorktreeAt(entry);
    if (chosen === "prune") await pruneWorktreeAt(entry);
    if (chosen === "repair") await repairWorktreeAt(entry);
    if (chosen === "lock") {
      await worktrees.lock(entry.path, null).catch((err) => errors.report(err, "Could not lock the worktree"));
    }
    if (chosen === "unlock") {
      await worktrees.unlock(entry.path).catch((err) => errors.report(err, "Could not unlock the worktree"));
    }
    if (chosen === "reveal") {
      const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
      await revealItemInDir(entry.path).catch((err) => errors.report(err, "Could not reveal the worktree"));
    }
  }

  /** A row of the Repositories list, open or closed (#36). */
  async function repoContext(row: ListedRepo, x: number, y: number) {
    const info = await desktop.load();
    repoTarget = { kind: "repository", root: row.root, overview: row.overview };
    const items = repoMenu(
      {
        kind: "repository",
        active: row.overview !== null && repo?.repo.valueOf() === row.overview.repo.valueOf(),
        open: row.overview !== null,
        missing: row.overview?.missing ?? false,
        pinned: row.pinned,
        group: repoGroups.groups.of[row.root] ?? UNGROUPED,
        groups: groupChoices(repoGroups.groups),
      },
      info,
    );
    await popupContextMenu(items, x, y).catch(() => {});
  }

  /** A submodule node: the same menu, with the four list-only items explained away. */
  async function moduleContext(row: import("$lib/module-tree").ModuleRow, x: number, y: number) {
    const top = submodules.ownerRoot;
    if (!top) return;
    const info = await desktop.load();
    const open = submodules.open === row.key;
    repoTarget = { kind: "submodule", root: `${top}/${row.key}`, row };
    const items = repoMenu(
      {
        kind: "submodule",
        active: open,
        open,
        missing: false,
        pinned: false,
        group: UNGROUPED,
        groups: [],
      },
      info,
    );
    await popupContextMenu(items, x, y).catch(() => {});
  }

  /** Returns true when the id belonged to the Repositories panel and was handled here. */
  function runRepoCommand(id: string): boolean {
    const target = repoTarget;
    const command = parseRepoCommand(id);
    if (!target || !command) return false;
    const { root } = target;

    if (command.kind === "move") {
      repoGroups.assign(root, command.group);
      return true;
    }
    if (command.kind === "move-new") {
      void prompt
        .ask({ title: "New Group", label: "Name", confirm: "Create", validate: () => null })
        .then((name) => {
          if (name === null) return;
          repoGroups.add(name);
          const created = repoGroups.groups.order.at(-1);
          if (created) repoGroups.assign(root, created);
        });
      return true;
    }

    const shell = (step: Promise<unknown>, failure: string) =>
      void step.catch((err) => errors.report(err, failure));
    switch (command.id) {
      case "repo-open":
        if (target.kind === "submodule") void openModule(target.row);
        else if (target.overview) void selectRepository(target.overview);
        else {
          repoList.opened(root);
          void activate(root);
        }
        return true;
      case "repo-open-folder":
        shell(fileMenus.openOnDesktop(root), "Could not open the folder");
        return true;
      case "repo-reveal":
        shell(fileMenus.revealOnDesktop(root), "Could not reveal the folder");
        return true;
      case "repo-terminal":
        shell(openInTerminal(root, settings.current.terminal), "Could not open a terminal");
        return true;
      case "repo-powershell":
        shell(fileMenus.openPowerShell(root), "Could not open PowerShell");
        return true;
      case "repo-git-shell":
        shell(fileMenus.openGitShell(root), "Could not open Git Bash");
        return true;
      case "repo-close":
        void closeListed(target);
        return true;
      case "repo-pull":
      case "repo-push":
        void syncListed(target, command.id === "repo-pull" ? "pull" : "push");
        return true;
      case "repo-pin":
        repoList.togglePin(root);
        return true;
      case "repo-rename":
        void renameListed(root, target.kind === "repository" ? target.overview?.name : undefined);
        return true;
      case "repo-remove":
        void removeListed(target);
        return true;
      default:
        return false;
    }
  }

  function isActive(overview: import("$lib/ipc").RepoOverview | null): boolean {
    return overview !== null && repo?.repo.valueOf() === overview.repo.valueOf();
  }

  /** The row stays in the list, closed; Remove is what takes it out. A submodule closes
      back to the repository it belongs to. */
  async function closeListed(target: RepoMenuSubject) {
    if (target.kind === "submodule") {
      const owner = repository.openRepos.find((entry) => entry.root === submodules.ownerRoot);
      if (owner) await selectRepository(owner);
      return;
    }
    const { overview } = target;
    if (!overview) return;
    repoList.closed(target.root);
    const wasActive = isActive(overview);
    if (wasActive) {
      commit.clear();
      diff.clear();
      health.clear();
    }
    await repository.closeOne(overview.repo);
    if (!wasActive) return;
    const next = repository.openRepos[0];
    if (next) await activate(next.root);
    else graph.clear();
  }

  /** Pull or push the row's repository without bringing it to the front. */
  async function syncListed(target: RepoMenuSubject, kind: "pull" | "push") {
    if (target.kind === "repository" && isActive(target.overview)) {
      await runNetwork(kind);
      return;
    }
    const id =
      target.kind === "repository"
        ? (target.overview?.repo ?? null)
        : ((await openedModule(target.row.key))?.repo ?? null);
    if (id === null) return;
    const remote = await primaryRemote(id);
    if (!remote) {
      errors.message("This repository has no remote.", `Could not ${kind}`);
      return;
    }
    try {
      if (kind === "pull") await network.pull(id, remote, true);
      else await network.push(id, remote, false);
    } catch (err) {
      errors.report(err, `Could not ${kind}`);
    }
    await repository.refreshList();
  }

  async function renameListed(root: string, folder: string | undefined) {
    const current = repoList.list.names[root] ?? folder ?? root;
    const name = await prompt.ask({
      title: "Rename",
      label: "Name shown in the list; the folder keeps its name",
      value: current,
      confirm: "Rename",
      validate: () => null,
    });
    if (name !== null) repoList.rename(root, name);
  }

  async function removeListed(target: RepoMenuSubject) {
    if (target.kind !== "repository") return;
    const name = repoList.list.names[target.root] ?? target.overview?.name ?? target.root;
    const yes = await confirmation.ask({
      title: "Remove",
      message: `Remove ${name} from the list? Nothing is deleted: the folder and the repository stay as they are.`,
      confirm: "Remove",
    });
    if (!yes) return;
    if (target.overview) {
      repoList.closed(target.root);
      await closeListed(target);
    }
    repoList.forget(target.root);
    repoGroups.assign(target.root, UNGROUPED);
  }

  /** Returns true when the id belonged to the References tree and was handled here. */
  function runRefCommand(id: string): boolean {
    const node = refTarget;
    if (!node) return false;

    switch (id) {
      case "restore-lost": {
        const found = recovery.lost.find((row) => row.oid === node.oid);
        if (found) void recoverCommit(found);
        return true;
      }
      case "copy-sha":
        if (node.oid) void copyText(node.oid);
        return true;
      default:
        return false;
    }
  }

  /** `ask` means the user has not decided: nothing is fetched and no cache is made. */
  $effect(() => {
    void avatars.apply(settings.current.avatars === "gravatar");
  });

  async function openSplit() {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    splitPublished = await isPublished(id, rev).catch(() => false);
    splitOpen = true;
  }

  async function runSplit(paths: string[], message: string, splitFirst: boolean) {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    splitBusy = true;
    try {
      await splitOff(id, rev, paths, message, splitFirst);
      splitOpen = false;
      await repository.refresh();
      commit.clear();
    } catch (err) {
      errors.report(err, "Could not split the commit");
    } finally {
      splitBusy = false;
    }
    await afterMutation();
  }

  async function browseForScan() {
    const picked = await openFolderDialog({ directory: true, title: "Scan Folder" });
    if (typeof picked !== "string") return;
    const watch = measure("scan-folder");
    await scan.run(picked, 6);
    watch.stop(`${scan.hits.length} repositories`);
  }

  /** Opens every chosen hit, then activates the first so the window is not left empty. */
  async function openScanned(roots: string[]) {
    opening = true;
    try {
      for (const root of roots) {
        await openRepository(root).catch((err) => errors.report(err, "Could not open the repository"));
      }
      await repository.refreshList();
      const first = roots[0];
      if (first) await activate(first);
    } finally {
      opening = false;
      scanOpen = false;
      scan.clear();
    }
  }

  /** The profile log is the answer to "why was that slow?": it has to be reachable. */
  async function revealLog() {
    if (info) await revealPath(info.logPath);
  }

  async function revealPath(path: string) {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path).catch(() => errors.report({ kind: "internal", data: path }, "Could not open the folder"));
  }

  /** The frontend list ships beside the page in a release build; the dev server has none. */
  async function showLicences() {
    const response = await fetch(`/${THIRD_PARTY_FILE}`).catch(() => null);
    const text = response?.ok ? await response.text() : "";
    const frontend = text.startsWith("Frontend packages") ? text : null;
    await openThirdPartyLicences(frontend).catch((err) =>
      errors.report(err, "Could not open the third-party licences"),
    );
  }

  /** The journal stays open: undoing one entry rarely means undoing only one. */
  async function undoEntry(entry: import("$lib/ipc").SafetyEntry) {
    const id = repo?.repo;
    if (!id) return;
    journalBusy = true;
    try {
      await safety.undoOne(id, entry.id);
      await repository.refresh();
      await afterWorkingTreeChange();
    } catch (err) {
      errors.report(err, "Could not undo");
    } finally {
      journalBusy = false;
    }
  }

  /** The root opened, or null when the dialog was cancelled or the open did not win. */
  async function pickRepository(): Promise<string | null> {
    const picked = await openFolderDialog({ directory: true, title: "Open Repository" });
    if (typeof picked !== "string") return null;
    opening = true;
    try {
      return await activate(picked);
    } finally {
      opening = false;
    }
  }

  /** Stages only the executable bit, leaving the edits in the working tree (T6.4). */
  async function stageModeOnly(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    for (const path of paths) {
      const file = worktree.unstaged.find((entry) => entry.path === path);
      if (!file?.modeChange) continue;
      await stageMode(id, path, file.modeChange === "executable").catch((err) =>
        errors.report(err, "Could not stage the mode change"),
      );
    }
    await afterWorkingTreeChange(paths);
  }

  /** Seeds an empty draft from `commit.template`, the way `git commit` would. */
  async function loadTemplate() {
    const id = repository.current?.repo;
    if (!id) return;
    const epoch = repository.epoch;
    const found = (await commitTemplate(id).catch(() => null)) ?? null;
    if (repository.epoch === epoch) template = found;
  }

  async function copyText(text: string) {
    if (text === "") return;
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
  }

  async function closeCurrent() {
    const id = repository.current?.repo;
    if (!id) return;
    const listed = repository.openRepos.find((entry) => entry.repo === id);
    if (listed) repoList.closed(listed.root);
    commit.clear();
    diff.clear();
    health.clear();
    await repository.closeOne(id);
  }

  /** Where the user was last: written as it changes, not only on the way out, because a
      crash is exactly the case this is meant to survive. */
  $effect(() => {
    const root = repository.current?.root;
    const oid = commit.oid;
    // Tracks the selection, not the session it writes it into (R-93).
    if (root) untrack(() => session.setSelected(root, oid));
  });

  /** Closing throws away whatever is only in the window: an edited hook, a resolution
      nobody wrote yet. Everything else is already on disk or in the draft store. */
  async function mayClose(): Promise<boolean> {
    const source = exitFlow.takeSource();
    session.persist();
    const what = unsavedSummary({
      hook: hooks.dirty ? hooks.editing : null,
      merge: conflicts.regions.length > 0 ? conflicts.path : null,
    });
    if (what && !(await ask(`${what} Close anyway?`, { title: "Cogit", kind: "warning" }))) {
      return false;
    }
    // Someone who just said "close anyway" has been asked once already.
    const confirm = what ? false : settings.current.confirmExit;
    return exitFlow.ask(source, confirm, listOperations);
  }

  /** No close request is pending here, so the window is destroyed rather than closed:
      closing would ask the same question a second time. */
  async function onSessionEnding() {
    session.persist();
    if (!(await exitFlow.ask("system", settings.current.confirmExit, listOperations))) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().destroy();
  }

  function answerExit(action: ExitAction, dontShowAgain: boolean) {
    const stored = exitFlow.answer(action, dontShowAgain, settings.current.confirmExit);
    if (stored !== null) void settings.set("confirmExit", stored);
  }

  const exitNames = $derived(
    new Map(repository.openRepos.map((entry) => [entry.repo.valueOf(), entry.name])),
  );

  /** A folder dropped on the window is a repository to open. Anything that is not one is
      refused by the backend and reported like any other failed open. */
  function onDragDrop(event: import("@tauri-apps/api/webview").DragDropEvent) {
    if (event.type === "enter") dropping = true;
    else if (event.type === "leave") dropping = false;
    else if (event.type === "drop") {
      dropping = false;
      void openDropped(droppedRepositories(event.paths));
    }
  }

  /** One subscription for everything the window hears from outside itself. */
  $effect(() =>
    connect({
      repoChanged: onDiskChange,
      operationChanged: (event) => {
        running = applyOperation(running, event);
        exitFlow.observe(event);
      },
      avatarReady: (email) => void avatars.refresh(email),
      mergeResolved: (event) => {
        if (repository.current?.repo.valueOf() !== event.repo.valueOf()) return;
        conflicts.close();
        void afterWorkingTreeChange();
      },
      commandRecorded: (event) => void output.notice(event),
      closeRequested: mayClose,
      sessionEnding: () => void onSessionEnding(),
      dragDrop: onDragDrop,
    }),
  );

  // A warning is worth a glance, not a dismissal: every commit on Windows produces one
  // about line endings, and a toast that waits to be clicked becomes a second thing to
  // clean up after each commit.
  $effect(() => {
    if (!output.warning) return;
    const timer = setTimeout(() => output.dismissWarning(), 8_000);
    return () => clearTimeout(timer);
  });

  /** Only the operations that mean the same thing when run again. Deleting a branch
      that is already gone is not a retry, it is a second, different failure. */
  function retryFor(entry: import("$lib/ipc").GitOutput): (() => void) | undefined {
    const kind = retryOf(entry, repository.current?.root ?? null, network.primary);
    if (kind === null) return undefined;
    return () => {
      output.close();
      void runNetwork(kind);
    };
  }

  async function openDropped(paths: string[]) {
    for (const path of paths.slice(1)) {
      await repository.open(path).catch((err) => errors.report(err, "Could not open the repository"));
    }
    const first = paths[0];
    if (first) await activate(first);
    else await repository.refreshList();
  }

  $effect(() => {
    const pending = onMenuCommand((id) => {
      if (id === "toolbar-preferences") return openSettings("toolbar");
      if (refActions?.run(id)) return;
      if (runGroupCommand(id)) return;
      if (runRepoCommand(id)) return;
      if (runFileCommand(id)) return;
      if (runRefCommand(id)) return;
      const command = palette.find((entry) => entry.id === id);
      if (command && !command.unavailable) runCommand(command);
    });
    return () => void pending.then((unlisten) => unlisten());
  });

  /** P0: what the renderer is holding, sampled into the profile log every ten seconds.
      Debug builds only — the DOM walk is not free, and nobody reads `kind=mem` in a
      release. The counters are read inside the callback, which runs off the interval and
      so outside the tracking context: the probe must not restart whenever the graph grows. */
  $effect(() => {
    if (!import.meta.env.DEV) return;

    return startMemoryProbe(
      browserSources(liveListeners, () => ({
        graphRows: graph.total,
        graphLoadedRows: graph.loadedRows(),
        avatarRows: avatars.rows.size,
        overlapRows: overlap.rows.size,
        diffHunks: diff.hunks.length,
        commitFiles: commit.files.length,
        outputEntries: output.entries.length,
        openRepos: repository.openRepos.length,
      })),
      (taken) => void reportMemory(taken),
    );
  });

  /** A rebuilt bar starts with every tick cleared, so this runs again after a keymap save. */
  function pushMenuState() {
    const checked = checkedIds({
      panels: PANELS.filter((panel) => layout.visible(panel)),
      output: output.open,
      maximized: layout.maximized !== null,
      overlap: overlap.enabled,
      avatars: avatars.enabled,
      perspective: layout.active,
    });
    void setMenuState(disabledIds(palette), checked).catch(() => {});
  }

  // The native menu is not reactive, so the derived state is pushed to it. muda flips a
  // tick itself when the item is clicked, so this also puts the wrong one back.
  $effect(pushMenuState);
</script>

<svelte:window
  {onkeydown}
  ondragover={(event) => {
    pointer.x = event.clientX;
    pointer.y = event.clientY;
  }}
/>

<TooltipLayer />

<div class="app">
  <Toolbar
    facts={toolbarFacts}
    menus={toolbarMenus}
    layout={toolbar.layout}
    oncontext={(x, y) =>
      void popupContextMenu(
        [{ id: "toolbar-preferences", label: "Toolbar Preferences…", enabled: true }],
        x,
        y,
      ).catch(() => {})}
    undoable={lastUndo?.description}
    handlers={repo
      ? {
          undo: () => void undo(),
          stash: stashAll,
          "stash-selection": () => void stashSelected(),
          "quick-stash-all": () => void quickStashAll(),
          "quick-stash-selection": () => void stashSelected(false),
          "apply-stash": () => void applyNewestStash(),
          tag: () => void refActions?.addTag(null),
          "push-to": () => refActions?.pushToCurrent(),
          pull: () => void pullNow(),
          push: () => void runNetwork("push"),
          sync: () => void syncNow(),
          "sync-order": (order) =>
            void syncNow(order === "pushThenPull" ? "pushThenPull" : "pullThenPush"),
          "fetch-remote": (remote) => void fetchRemotes(remote ? [remote] : []),
          "fetch-remotes": () => void fetchRemotes(network.remotes),
          "pull-scope": (scope) =>
            void toolbar.set("pullScope", scope === "all" ? "all" : "current"),
          "delete-merged": () =>
            void toolbar.set("deleteMergedAfterPull", !toolbar.prefs.deleteMergedAfterPull),
          stage: () => void stage(targetsOf("stage", toolbarFacts)),
          unstage: () => void unstage(targetsOf("unstage", toolbarFacts)),
          discard: () => void discardFromToolbar(targetsOf("discard", toolbarFacts)),
          merge: () => void mergeSelected(),
          rebase: () => void rebaseSelected(),
          "rebase-i": () => void openRebase(),
        }
      : {}}
  />

  {#if banner && !shown.graph}
    <StateBanner {banner} busy={repository.busy} onaction={runBannerAction} />
  {/if}

  <div class="workspace" bind:clientWidth={workspaceWidth}>
    {#if leftColumn}
    <div
      class="left-column"
      style:flex={shown.diff || topRow ? `0 1 ${fractions.leftColumn * 100}%` : "1 1 auto"}
    >
      {#if reposColumn}
      <div
        class="repos-column"
        class:grow={!shown.refs}
        style:flex={shown.refs ? `0 0 ${fractions.repositories * 100}%` : undefined}
        bind:clientHeight={reposColumnHeight}
      >
      {#if shown.repositories}
      <div
        class="pane"
        class:grow={!shown.worktrees}
        style:flex={shown.worktrees ? `0 1 ${fractions.worktrees * 100}%` : undefined}
        role="region"
        aria-label={PANEL_TITLES.repositories}
        onpointerdown={() => (focused = "repositories")}
      >
        <Panel
          title="Repositories"
          active={focused === "repositories"}
          count={repository.openRepos.length}
          stale={stale.has("repositories")}
        >
          <RepositoriesPanel
            {opening}
            onscan={() => {
              scanOpen = true;
              void browseForScan();
            }}
            onopen={pickRepository}
            onselect={(entry) => void selectRepository(entry)}
            onclose={(entry) => void closeListed({ kind: "repository", root: entry.root, overview: entry })}
            oncontext={(row, x, y) => void repoContext(row, x, y)}
            onreopen={(root) => void activate(root)}
            onmarked={(roots) => (markedRepos = roots)}
            onaddgroup={askAddGroup}
            ongroupcontext={(id, x, y) => void groupContext(id, x, y)}
            onopenmodule={(row) => void openModule(row)}
            onmodulecontext={(row, x, y) => void moduleContext(row, x, y)}
          />
        </Panel>
      </div>
      {/if}
      {#if shown.repositories && shown.worktrees}
      <Splitter
        direction="horizontal"
        value={fractions.worktrees}
        label="Resize worktrees panel"
        onchange={(d) =>
          layout.set("worktrees", capFraction(fractions.worktrees + d, reposColumnHeight, WORKTREES_MIN_PX))}
        onreset={() => layout.resetOne("worktrees")}
      />
      {/if}
      {#if shown.worktrees}
      <div
        class="pane grow worktrees-pane"
        role="region"
        aria-label={PANEL_TITLES.worktrees}
        onpointerdown={() => (focused = "worktrees")}
      >
        <Panel
          title="Worktrees"
          active={focused === "worktrees"}
          view={panelState}
          count={worktrees.entries.length > 1 ? worktrees.entries.length : undefined}
        >
          {#snippet actions()}
            {#if repo}
              <button type="button" class="panel-act" title="Add Worktree…" onclick={() => (addWorktreeOpen = true)}
                >Add…</button
              >
              <button
                type="button"
                class="panel-act"
                disabled={!hasStale(worktrees.entries)}
                title={hasStale(worktrees.entries)
                  ? "Forget every worktree whose folder is gone"
                  : "No worktree is missing"}
                onclick={() => void pruneWorktreesHere()}>Prune All</button
              >
            {/if}
          {/snippet}
          <WorktreesPanel
            onopen={(entry) => void openWorktreeRow(entry)}
            oncontext={(entry, x, y) => void worktreeContext(entry, x, y)}
            onprune={(entry) => void pruneWorktreeAt(entry)}
            onrepair={(entry) => void repairWorktreeAt(entry)}
            onadd={() => (addWorktreeOpen = true)}
          />
        </Panel>
      </div>
      {/if}
      </div>
      {/if}
      {#if reposColumn && shown.refs}
      <Splitter
        direction="horizontal"
        value={fractions.repositories}
        label="Resize repositories panel"
        onchange={(d) => layout.nudge("repositories", d)}
        onreset={() => layout.resetOne("repositories")}
      />
      {/if}
      {#if shown.refs}
      <div class="pane grow" role="region"
        aria-label={PANEL_TITLES.refs}
        onpointerdown={() => (focused = "refs")}>
        <Panel
          title="Branches"
          active={focused === "refs"}
          stale={stale.has("refs")}
          view={panelState}
          count={repo?.branches.length}
        >
          {#snippet actions()}
            {#if repo}
              <input
                class="ref-filter"
                type="search"
                bind:value={refFilter}
                placeholder="Filter refs or oid"
                aria-label="Filter references"
              />
              <RefSortButtons />
            {/if}
          {/snippet}
          <ReferencesPanel
            input={refTreeInput}
            onvisible={() => void reloadGraph()}
            onselect={selectRef}
            oncheckout={switchTo}
            onactivate={activateRef}
            oncontext={(node, x, y) => void refContext(node, x, y)}
            ondrop={onBranchDrop}
          />
        </Panel>
      </div>
      {/if}
    </div>
    {/if}

    {#if leftColumn && (topRow || shown.diff)}
    <Splitter
      direction="vertical"
      value={fractions.leftColumn}
      label="Resize left column"
      onchange={(d) =>
        layout.set("leftColumn", capFraction(fractions.leftColumn + d, workspaceWidth, graphMinPx()))}
      onreset={() => layout.resetOne("leftColumn")}
    />
    {/if}

    {#if topRow || shown.diff}
    <div class="right-area" style:min-width={shown.graph ? graphMin : undefined}>
      {#if topRow}
      <div
        class="top-row"
        bind:clientWidth={topRowWidth}
        style:flex={shown.diff ? `0 0 ${fractions.topRow * 100}%` : "1 1 auto"}
      >
        {#if shown.graph}
        <div
          class="pane"
          class:grow={!shown.files}
          style:flex={shown.files ? `0 0 ${fractions.graph * 100}%` : undefined}
          style:min-width={graphMin}
          bind:this={graphPane}
          role="region"
        aria-label={PANEL_TITLES.graph}
        onpointerdown={() => (focused = "graph")}
        >
          <Panel
            title="Graph &amp; History"
            active={focused === "graph"}
            view={panelState}
            count={graph.total}
            stale={stale.has("graph")}
          >
            {#snippet actions()}
              {#if repo}
                <GraphFilter onchange={filterGraph} matches={graph.total} />
              {/if}
            {/snippet}
            {#if describeSkipped(graph.skipped)}
              <p class="graph-skipped" role="status">{describeSkipped(graph.skipped)}</p>
            {/if}
            <GraphPanel
              recent={session.recent}
              onopenrecent={(path) => void activate(path)}
              onforgetrecent={(path) => session.forgetRecent(path)}
              onopen={pickRepository}
              onscan={() => (scanOpen = true)}
              {progress}
              check={checkCommand}
              oncheck={(command) => (checkCommand = command)}
              onruncheck={() => void runPauseCheck()}
              verdict={checkVerdict}
              {checking}
              ondrop={onCommitDrop}
              oncontext={(oid, x, y) => void commitContext(oid, x, y)}
              {banner}
              busy={repository.busy}
              onbanneraction={runBannerAction}
              onworktreecontext={(x, y) => void refActions?.worktreeContext(x, y)}
              onrefcontext={(label, oid, x, y) => void refActions?.labelContext(label, oid, x, y)}
            />
          </Panel>
        </div>
        {/if}
        {#if shown.graph && filesColumn}
        <Splitter
          direction="vertical"
          value={fractions.graph}
          label="Resize graph panel"
          onchange={(d) =>
            layout.set("graph", floorFraction(fractions.graph + d, topRowWidth, graphMinPx()))}
          onreset={() => layout.resetOne("graph")}
        />
        {/if}
        {#if filesColumn}
        <div
          class="files-column"
          bind:clientHeight={filesColumnHeight}
          class:grow={!shown.graph}
          style:flex={shown.graph ? `1 1 auto` : undefined}
        >
        {#if shown.files}
        <div
          class="pane"
          class:grow={!(shown.commit && onWorkingTree)}
          style:flex={shown.commit && onWorkingTree ? `0 1 ${fractions.commitBox * 100}%` : undefined}
          role="region"
          aria-label={PANEL_TITLES.files}
          onpointerdown={() => (focused = "files")}>
          <Panel
            title="Files"
            active={focused === "files"}
            view={panelState}
            count={onWorkingTree ? worktree.total : commit.files.length}
            stale={stale.has("files")}
          >
            <FilesPanel
              view={panelView(repository.phase)}
              activePanel={focused === "files"}
              {onWorkingTree}
              onviewchange={(next) => {
                filesView.set(next);
                if (repo) void worktree.load(repo.repo);
              }}
              onopenworktree={openWorktreeDiff}
              onopenstaged={openStagedDiff}
              onopencommit={openDiff}
              onopenstash={openStashDiff}
              onopenwindow={openInWindow}
              onmask={(mask) => (fileMask = mask)}
              onshownstaged={(paths) => (shownStaged = paths)}
              onmarked={(paths) => (markedFiles = paths)}
              oncontext={fileContext}
              {stage}
              stagemode={stageModeOnly}
              {unstage}
              {discard}
              {ignore}
              remove={deleteFromDisk}
            />
          </Panel>
        </div>
        {/if}

        {#if shown.files && shown.commit && onWorkingTree}
        <Splitter
          direction="horizontal"
          value={fractions.commitBox}
          label="Resize commit message panel"
          onchange={(d) =>
            layout.set("commitBox", capFraction(fractions.commitBox + d, filesColumnHeight, COMMIT_MIN_PX))}
          onreset={() => layout.resetOne("commitBox")}
        />
        {/if}

        {#if shown.commit && onWorkingTree}
        <div class="pane grow commit-pane" role="region"
          aria-label={PANEL_TITLES.commit}
          onpointerdown={() => (focused = "commit")}>
          <Panel
            title="Commit Message"
            active={focused === "commit"}
            view={panelState}
            count={worktree.staged.length}
            stale={stale.has("commit")}
          >
            <CommitPanel {scope} {template} oncommit={commitStaged} />
          </Panel>
        </div>
        {/if}
        </div>
        {/if}
      </div>
      {/if}

      {#if topRow && shown.diff}
      <Splitter
        direction="horizontal"
        value={fractions.topRow}
        label="Resize diff panel"
        onchange={(d) => layout.nudge("topRow", d)}
        onreset={() => layout.resetOne("topRow")}
      />
      {/if}

      {#if shown.diff}
      <div class="pane grow" role="region"
        aria-label={PANEL_TITLES.diff}
        onpointerdown={() => (focused = "diff")}>
        <Panel title="Diff" active={focused === "diff"} view={panelState} stale={stale.has("diff")}>
          <DiffPanel
            oninitsubmodule={(path) => void initSubmoduleAt(path)}
            onstage={(selected, reverse) => void stageLines(selected, reverse)}
            onblame={() => void showBlame()}
            onwhitespace={(mode) => {
              const id = repository.current?.repo;
              if (id) void diff.setWhitespace(id, mode);
            }}
            onexpand={(whole) => {
              const id = repository.current?.repo;
              if (id) void diff.expand(id, whole);
            }}
            onresolve={(side) => {
              const id = repository.current?.repo;
              if (id) void conflicts.take(id, side).then(() => afterWorkingTreeChange());
            }}
            onpopoutmerge={() => {
              const id = repository.current?.repo;
              if (id && conflicts.path) void openMergeWindow(id, conflicts.path);
            }}
            onresolveText={(text) => {
              const id = repository.current?.repo;
              if (id) void conflicts.write(id, text).then(() => afterWorkingTreeChange());
            }}
          >
            {#snippet fallback()}
              <CommitDetailsPane />
            {/snippet}
          </DiffPanel>
        </Panel>
      </div>
      {/if}
    </div>
    {/if}
  </div>

  {#if output.open}
    <OutputPanel />
  {/if}

  {#if finderOpen}
    <FindObject
      results={finder.results}
      busy={finder.busy}
      onquery={(text) => void runFind(text)}
      onpick={pickFound}
      onclose={() => (finderOpen = false)}
    />
  {/if}

  {#if paletteOpen}
    <CommandPalette
      commands={palette}
      recent={recentCommands}
      onrun={runCommand}
      onclose={() => (paletteOpen = false)}
    />
  {/if}

  {#if dropMenu}
    <DropMenu
      actions={dropMenu.actions}
      x={dropMenu.x}
      y={dropMenu.y}
      onpick={(action) => void runDropAction(action)}
      onclose={() => (dropMenu = null)}
    />
  {/if}

  {#if rebaseOpen}
    <RebaseEditor
      base={rebaseBase}
      plan={rebasePlan}
      published={splitPublished}
      busy={rebaseBusy}
      onplan={(next) => (rebasePlan = next)}
      paused={rebasePaused}
      onpaused={(next) => (rebasePaused = next)}
      onrun={() => void runRebase()}
      onclose={() => (rebaseOpen = false)}
    />
  {/if}

  <RefActions
    bind:this={refActions}
    {afterRefChange}
    afterMutation={() => afterMutation()}
    {reloadGraph}
    checkoutBranch={switchTo}
    {openSplit}
    {openRebase}
    rollbackTree={() => rollbackFiles([])}
  />

  {#if splitOpen && commit.oid}
    <SplitOffDialog
      oid={commit.oid}
      changed={commit.files.map((file) => file.path)}
      published={splitPublished}
      busy={splitBusy}
      onsplit={(paths, message, first) => void runSplit(paths, message, first)}
      onclose={() => (splitOpen = false)}
    />
  {/if}

  {#if hooks.open}
    <HooksPanel
      overview={hooks.overview}
      editing={hooks.editing}
      body={hooks.body}
      onedit={(name) => {
        const id = repository.current?.repo;
        if (id) void hooks.edit(id, name);
      }}
      onbody={(text) => (hooks.body = text)}
      onsave={() => {
        const id = repository.current?.repo;
        if (id) void hooks.save(id);
      }}
      oncancel={() => (hooks.editing = null)}
      ontoggle={(name, enabled) => {
        const id = repository.current?.repo;
        if (id) void hooks.toggle(id, name, enabled);
      }}
      onadopt={(path) => {
        const id = repository.current?.repo;
        if (id) void hooks.adopt(id, path);
      }}
      onrun={(name) => {
        const id = repository.current?.repo;
        if (id) void hooks.dryRun(id, name);
      }}
      lastRun={hooks.lastRun}
      running={hooks.running}
      bypasses={hooks.bypasses}
      presets={hooks.presets}
      showPresets={hooks.showPresets}
      ontogglepresets={() => (hooks.showPresets = !hooks.showPresets)}
      oninstall={(id) => {
        const repo = repository.current?.repo;
        if (repo) void hooks.install(repo, id);
      }}
      onexport={(hook) => {
        const repo = repository.current?.repo;
        if (!repo) return;
        void prompt
          .ask({
            title: "Save as preset",
            label: `A name for the preset made from ${hook}`,
            value: hook,
            confirm: "Save",
            validate: textProblem,
          })
          .then((name) => {
            if (name !== null) void hooks.export(repo, hook, name);
          });
      }}
      onremovepreset={(id) => {
        const repo = repository.current?.repo;
        if (repo) void hooks.removeOwn(repo, id);
      }}
      onclose={() => hooks.close()}
    />
  {/if}

  {#if settingsOpen}
    <SettingsPanel
      value={settings.current}
      {terminals}
      keymap={settings.keymap}
      bindings={settings.bindings}
      tokenHost={network.tokenHost}
      tokenStored={network.tokenStored}
      onstoretoken={(token) => void network.storeToken(token)}
      onforgettoken={() => void network.forgetToken()}
      onapply={(next, keys) => void applySettings(next, keys)}
      onrevert={() => void revertSettings()}
      onclose={() => (settingsOpen = false)}
      ignored={health.ignored}
      onunignore={(root, warning) => void health.unignore(root, warning)}
      start={settingsStart}
      toolbarLayout={toolbar.layout}
      ontoolbar={(next) => void toolbar.setLayout(next)}
    />
  {/if}

  {#if configEdit}
    <ConfigEditor
      title={configEdit.scope === "repository" ? "Edit Git Config — Repository" : "Edit Git Config — User"}
      file={configEdit.file}
      problem={configEdit.problem}
      saving={configEdit.saving}
      onsave={(text) => void saveConfig(text)}
      oncancel={() => (configEdit = null)}
    />
  {/if}

  {#if exitFlow.prompt}
    <ExitDialog
      source={exitFlow.prompt.source}
      variant={exitFlow.variant}
      rows={exitRows(exitFlow.pending.values(), exitNames, {
        repo: network.running ? network.repo : null,
        line: network.progress,
      })}
      waiting={exitFlow.waiting}
      dontShow={!settings.current.confirmExit}
      onanswer={answerExit}
    />
  {/if}

  <Notifications
    oncopy={(text) => void copyText(text)}
    onopenurl={(url) => void import("@tauri-apps/plugin-opener").then((opener) => opener.openUrl(url))}
    onshowoutput={(record) => void output.openRecord(record)}
    onaction={(action) => void runNoticeAction(action)}
  />

  {#if aboutOpen && info}
    <AboutDialog
      {info}
      update={lastUpdate}
      onclose={() => (aboutOpen = false)}
      oncopy={(text) => void copyText(text)}
      onreveal={(path) => void revealPath(path)}
      onlicences={() => void showLicences()}
      oncheckupdates={() => void runUpdateCheck()}
    />
  {/if}

  {#if journalOpen}
    <SafetyJournal
      entries={repo ? safety.forRepo(repo.repo) : []}
      busy={journalBusy}
      onundo={(entry) => void undoEntry(entry)}
      onclose={() => (journalOpen = false)}
    />
  {/if}

  <StashDialogs />

  {#if remoteOps.dialog}
    {#key remoteOps.dialog}
      <RemoteOpsDialog
        spec={remoteOps.dialog.spec}
        link={remoteOps.dialog.link}
        onsubmit={(values) => void remoteOps.submit(values)}
        onclose={() => remoteOps.close()}
      />
    {/key}
  {/if}

  {#if repoSettingsOpen && repo}
    <!-- Readers of these keys, the tag folders among them, pick them up on the refresh. -->
    <RepoSettingsDialog
      repo={repo.repo}
      name={repo.name}
      onsaved={() => repository.refresh()}
      onclose={() => (repoSettingsOpen = false)}
    />
  {/if}

  {#if prompt.open}
    <PromptDialog
      title={prompt.open.title}
      label={prompt.open.label}
      value={prompt.open.value ?? ""}
      choices={prompt.open.choices}
      confirm={prompt.open.confirm}
      validate={prompt.open.validate}
      onaccept={(value) => prompt.accept(value)}
      onclose={() => prompt.cancel()}
    />
  {/if}

  {#if removingFiles}
    <RemoveFilesDialog
      paths={removingFiles}
      onremove={(paths, deleteLocal) => void removeFiles(paths, deleteLocal)}
      onclose={() => (removingFiles = null)}
    />
  {/if}

  {#if indexEditing}
    <IndexEditorDialog
      path={indexEditing.path}
      sides={indexEditing.sides}
      saving={indexEditing.saving}
      onsave={(edited) => void saveIndexEditor(edited)}
      onclose={() => (indexEditing = null)}
    />
  {/if}

  {#if addWorktreeOpen && repo}
    <AddWorktreeDialog
      choices={branchChoices(repo.branches, worktrees.entries)}
      base={commit.oid ?? "HEAD"}
      onbrowse={async () => {
        const picked = await openFolderDialog({ directory: true, title: "Folder for the new worktree" });
        return typeof picked === "string" ? picked : null;
      }}
      onadd={(request) => void addWorktreeFrom(request)}
      onclose={() => (addWorktreeOpen = false)}
    />
  {/if}

  {#if worktreeRemoval}
    <RemoveWorktreeDialog
      entry={worktreeRemoval.entry}
      changes={worktreeRemoval.changes}
      onremove={(force) => void confirmWorktreeRemoval(force)}
      onclose={() => (worktreeRemoval = null)}
    />
  {/if}

  {#if scanOpen}
    <ScanDialog
      busy={opening}
      onbrowse={() => void browseForScan()}
      onopen={(roots) => void openScanned(roots)}
      onclose={() => {
        scanOpen = false;
        scan.clear();
      }}
    />
  {/if}

  {#if output.shown}
    <CommandOutput
      entry={output.shown}
      logPath={info?.logPath ?? ""}
      onretry={retryFor(output.shown)}
    />
  {/if}

  {#if output.warning}
    <div class="toast" role="status">
      <span class="what">{output.warning.operation}</span>
      <span class="said truncate">{output.warning.summary}</span>
      <button type="button" onclick={() => void output.showWarning()}>Details</button>
      <button type="button" onclick={() => output.dismissWarning()} title="Dismiss">✕</button>
    </div>
  {/if}

  <StatusBar
    repository={footerRepository(repository.phase)}
    branch={repo ? repository.headLabel : undefined}
    upstream={tracked?.upstream ?? undefined}
    ahead={tracked?.ahead ?? 0}
    behind={tracked?.behind ?? 0}
    summary={repo ? `${graph.total} commits · ${repo.branches.length} refs` : undefined}
    fileOpen={diff.path !== null}
    {...fileFormat(diff.diff)}
    activity={activity({
      operations: running,
      bulk,
      network: network.running ?? undefined,
      networkProgress: network.progress ?? undefined,
      opening: repository.busy,
      failed: notices.errorCount > 0,
    })}
    problems={output.problems}
    onproblems={() => output.toggle()}
  />

  {#if dropping}
    <div class="drop-hint" aria-hidden="true">Drop a folder to open it as a repository</div>
  {/if}
</div>

<style>
  .graph-skipped {
    margin: 0;
    padding: var(--sp-2) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    border-left: 3px solid var(--status-modify);
    background: var(--surface-raised);
    color: var(--text-primary);
    font-size: var(--fs-dense);
  }

  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-base);
  }

  /* A warning interrupts nothing: it sits in the corner and leaves by itself. */
  .toast {
    position: absolute;
    right: var(--sp-5);
    bottom: calc(var(--h-statusbar) + var(--sp-4));
    z-index: 20;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    max-width: min(560px, 60vw);
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-raised);
    border: 1px solid var(--status-modify);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-dialog);
    font-size: var(--fs-dense);
  }

  .toast .what {
    font-weight: 600;
  }

  .toast .said {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
  }

  .toast button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .drop-hint {
    position: absolute;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--surface-base) 78%, transparent);
    border: 2px dashed var(--status-ref);
    color: var(--text-primary);
    font-size: var(--fs-ui);
    pointer-events: none;
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

  /* A panel squeezed to its header is a panel the user cannot get back without the
     keyboard, so every one keeps room for a row or two. */
  .pane {
    display: flex;
    min-width: 0;
    min-height: calc(var(--h-panel-hdr) + 3 * var(--h-row-dense));
  }

  .files-column {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  /* Header, two lines of message and the Commit row; the splitter stops here (R-183). */
  .commit-pane {
    min-height: var(--commit-panel-min);
  }

  .repos-column {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .worktrees-pane {
    min-height: var(--worktrees-panel-min);
  }

  .panel-act {
    height: 18px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: 10px;
    cursor: default;
  }

  .panel-act:hover:not(:disabled) {
    border-color: var(--status-ref);
  }

  .panel-act:disabled {
    opacity: 0.45;
  }

  .pane > :global(.panel) {
    flex: 1 1 auto;
  }

  .grow {
    flex: 1 1 0;
  }

  .ref-filter {
    width: 140px;
    height: 18px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: 10px;
  }

  /* Raw Git output is never reformatted or truncated (INV-05). */

</style>
