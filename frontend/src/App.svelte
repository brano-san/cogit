<script lang="ts">
  import { tick, untrack } from "svelte";
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { checkForUpdates, message, PORTABLE_UPDATE_NOTE, type UpdateOutcome } from "$lib/updates";
  import { leaveRepositoryDialogs } from "$lib/leaving";
  import { retryOf } from "$lib/retry";
  import { parseQuery } from "$lib/query";
  import { publishedOrAssume } from "$lib/published";
  import { menuStatePusher } from "$lib/menu-state";
  import { branchNameProblem, optional, presetNameProblem, textProblem } from "$lib/names";
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
  import GraphHeader from "$components/graph/GraphHeader.svelte";
  import RebaseEditor from "$components/graph/RebaseEditor.svelte";
  import DropMenu from "$components/layout/DropMenu.svelte";
  import Panel from "$components/layout/Panel.svelte";
  import CommandPalette from "$components/layout/CommandPalette.svelte";
  import AboutDialog from "$components/common/AboutDialog.svelte";
  import ExitDialog from "$components/common/ExitDialog.svelte";
  import ConfigEditor from "$components/common/ConfigEditor.svelte";
  import Notifications from "$components/layout/Notifications.svelte";
  import SuccessToast from "$components/layout/SuccessToast.svelte";
  import { successToast } from "$stores/success-toast.svelte";
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
  import { suppressBrowserFind } from "$lib/browser-find";
  import { suppressBrowserNavigation } from "$lib/browser-navigation";
  import { findModuleRow, isModulePath, onOpenModule } from "$lib/module-open";
  import { onTreeChange } from "$lib/tree-sync";
  import { emptyStateVisible, footerRepository, panelView } from "$lib/repo-phase";
  import StartScreen from "$components/layout/StartScreen.svelte";
  import { flushTrace, startTracing, timed, trace } from "$lib/trace";
  import OutputPanel from "$components/layout/OutputPanel.svelte";
  import StateBanner from "$components/layout/StateBanner.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import StatusBar from "$components/layout/StatusBar.svelte";
  import Toolbar from "$components/layout/Toolbar.svelte";
  import ScanDialog from "$components/repo-tree/ScanDialog.svelte";
  import CloneDialog from "$components/repo-tree/CloneDialog.svelte";
  import WelcomeDialog from "$components/layout/WelcomeDialog.svelte";
  import { welcome } from "$stores/welcome.svelte";
  import { webMenus } from "$stores/web-menus.svelte";
  import {
    folderPlan,
    mruRows,
    pathKey,
    shouldShowAtStartup,
    WELCOME_FORGET,
    type WelcomeAction,
  } from "$lib/welcome";
  import { folderKind, initRepository } from "$lib/ipc/clone";
  import { cloneWizard } from "$stores/clone.svelte";
  import { parentFolder, runClone } from "$lib/clone";
  import { cloneRepository, type CloneRequest } from "$lib/ipc/clone";
  import PromptDialog from "$components/layout/PromptDialog.svelte";
  import StashDialogs from "$components/layout/StashDialogs.svelte";
  import GitMissingDialog from "$components/layout/GitMissingDialog.svelte";
  import { GitMissingStore } from "$stores/git-missing.svelte";
  import { gitReasonOf } from "$lib/git-missing";
  import RemoteOpsDialog from "$components/remote/RemoteOpsDialog.svelte";
  import RepoSettingsDialog from "$components/remote/RepoSettingsDialog.svelte";
  import { remoteCommands, submoduleScope } from "$lib/remote-menu";
  import { remoteOps } from "$stores/remote-ops.svelte";
  import RemoveFilesDialog from "$components/file-list/RemoveFilesDialog.svelte";
  import IndexEditorDialog from "$components/file-list/IndexEditorDialog.svelte";
  import { openInvestigate } from "$lib/investigate/open";
  import WorktreesPanel from "$components/panels/WorktreesPanel.svelte";
  import { hasStale, othersToWatch, removable } from "$lib/worktree-list";
  import { fileFormat, shortOid } from "$lib/format";
  import { checkedIds, disabledIds, rememberCommand, type PaletteCommand } from "$lib/palette";
  import type { Context } from "$lib/availability";
  import { localRevision, reasonOf, refAt, splitMarked, targetsOf, type MenuContext, type ToolbarFacts } from "$lib/toolbar";
  import { toolbar } from "$stores/toolbar.svelte";
  import { stashDialog } from "$stores/stash-dialog.svelte";
  import { settle, step } from "$lib/panel-focus";
  import { installSelectAll, selectAllFromMenu } from "$lib/select-all";
  import { needsPush, pullRequestFor } from "$lib/pull-request";
  import { commitScope } from "$lib/commit-scope";
  import { activity, applyOperation, cancellable, trackCancellable } from "$lib/operations";
  import { measurer } from "$lib/timing";
  import { item as menuItem, refMenu } from "$lib/context-menu";
  import RefActions from "$components/menus/RefActions.svelte";
  import RefGroupActions from "$components/menus/RefGroupActions.svelte";
  import SelectionActions from "$components/menus/SelectionActions.svelte";
  import BisectActions from "$components/menus/BisectActions.svelte";
  import WorktreeActions from "$components/menus/WorktreeActions.svelte";
  import NetworkActions from "$components/menus/NetworkActions.svelte";
  import { bisectCommands } from "$lib/bisect";
  import { appCommands, PANEL_TITLES, type AppCommandActions } from "$lib/app-commands";
  import { compareView } from "$stores/compare-view.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { undoRewriteQuestion } from "$lib/undo-rewrite";
  import { isAncestor, undoRewrite, undoRewriteInfo } from "$lib/ipc/ref-ops";
  import { effective, withShortcuts } from "$lib/keymap";
  import { binName, ON_MAC, OS, primary } from "$lib/platform";
  import { menuCommandRuns, modals } from "$lib/modal-stack";
  import { commitBox } from "$stores/commit-box.svelte";
  import { commitFileMenu, shownRow, worktreeFileMenu } from "$lib/file-menu";
  import { fileName, runFileMenuCommand, type FileActions, type FileScope } from "$lib/file-actions";
  import { listedMessage } from "$lib/file-dialogs";
  import * as fileMenus from "$lib/ipc/file-menus";
  import { desktop } from "$stores/desktop.svelte";
  import { groupChoices, parseRepoCommand, repoMenu } from "$lib/repo-menu";
  import { listedName, listedRepos, type ListedRepo } from "$lib/repo-list";
  import { rowSync, summaryPulse } from "$lib/repo-sync";
  import { removalQuestion, UNGROUPED } from "$lib/repo-groups";
  import { repoList } from "$stores/repo-list.svelte";
  import { compareUrl, diffWindowTitle } from "$lib/compare-params";
  import { dropActions, type DropAction, type DragPayload } from "$lib/drop-target";
  import { moveEntry, planPublished } from "$lib/rebase-plan";
  import { splitRequest } from "$lib/split-off";
  import { readsAgain } from "$lib/file-view";
  import { abortAction, bannerQuestion, stateBanner, type BannerAction } from "$lib/repo-state";
  import { runCheckout } from "$lib/checkout-flow";
  import { autostashDialog } from "$stores/autostash-dialog.svelte";
  import { networkDialog } from "$stores/network-dialog.svelte";
  import { trackedRemote } from "$lib/network-runs";
  import { refActivation, type CheckoutRequest } from "$lib/ref-checkout";
  import { foundStep } from "$lib/found";
  import { revealRef } from "$lib/ref-reveal";
  import { applyPreferences, type ApplyHost } from "$lib/preferences-apply";
  import { capFraction, floorFraction, PANELS, worktreesHeightAfterDrag, type PanelId } from "$lib/perspectives";
  import { graphPanelMinWidth } from "$lib/graph-panel";
  import { closeStep, holdsPanels, reopenClick, repoClick } from "$lib/repo-click";
  import { ModuleInitialiser, moduleClick } from "$lib/module-init";
  import { moduleRoot, shownRowRoot, updateModule } from "$lib/module-tree";
  import { moduleForest } from "$stores/module-forest.svelte";
  import { answerMergeResolved } from "$lib/merge-save";
  import { browserSources, start as startMemoryProbe } from "$lib/mem-probe";
  import { liveListeners } from "$lib/listener-count";
  import type { Settings } from "$lib/settings";
  import {
    checkout,
    listHooks,
    rangeDiff,
    runMaintenance,
    addToExclude,
    ignoreRules,
    unportablePaths,
    abortOperation,
    continueOperation,
    createBranch,
    getAppInfo,
    openThirdPartyLicences,
    addToGitignore,
    closeThisWindow,
    interactiveRebase,
    isPublished,
    protectingRefs,
    rebaseProgress,
    rebaseTodo,
    rollbackTo,
    splitOff,
    switchWithAutostash as runSwitchWithAutostash,
    commitFiles as readCommitFiles,
    mergeInto,
    stashSelection,
    fetchRemote,
    onMenuCommand,
    openInTerminal,
    runCheck,
    terminalChoices,
    openRepository,
    openSubmodule,
    updateSubmodule,
    listSubmodules,
    openWorktree,
    listOperations,
    cancelNetwork,
    readGitConfig,
    writeGitConfig,
    reportMemory,
    reportTiming,
    commitTemplate,
    openCommitWindow,
    stageMode,
    openSolverWindow,
    openCompareWindow,
    popupContextMenu,
    resolveConflict,
    setMenuState,
    rebaseOnto,
    skipOperation,
    removeIndexLock,
    trustDirectory,
    showRepository,
    probeGit,
    findGitCandidates,
    useGit,
    type AppInfo,
    type Branch,
    type RepoId,
  } from "$lib/ipc";
  import { openBlame } from "$lib/blame-window";
  import { ShownRepository } from "$lib/shown-repository";
  import { commit } from "$stores/commit.svelte";
  import { commitTree } from "$stores/commit-tree.svelte";
  import { conflicts } from "$stores/conflicts.svelte";
  import { worktree } from "$stores/worktree.svelte";
  import { runMutation, type MutationContext } from "$lib/mutation";
  import { worktrees } from "$stores/worktrees.svelte";
  import { diff } from "$stores/diff.svelte";
  import { errors } from "$stores/errors.svelte";
  import { errorWindow } from "$stores/error-window.svelte";
  import { notices } from "$stores/notices.svelte";
  import { FETCH_MODULES, FIX_GIT, MAINTENANCE, RUN_GC, TRUST_DIRECTORY } from "$lib/health";
  import { hooksNote, pendingHooks } from "$lib/pending-hooks";
  import { output } from "$stores/output.svelte";
  import { taskbar } from "$stores/taskbar.svelte";
  import { network } from "$stores/network.svelte";
  import { recovery } from "$stores/recovery.svelte";
  import { safety } from "$stores/safety.svelte";
  import { stashes } from "$stores/stashes.svelte";
  import { submodules } from "$stores/submodules.svelte";
  import { moduleMemory } from "$stores/module-memory.svelte";
  import { repoPulse } from "$stores/repo-pulse.svelte";
  import { repoMenuRow } from "$stores/menu-row.svelte";
  import { graph } from "$stores/graph.svelte";
  import { hooks } from "$stores/hooks.svelte";
  import { avatars } from "$stores/avatars.svelte";
  import { prompt } from "$stores/prompt.svelte";
  import { session } from "$stores/session.svelte";
  import { flow } from "$stores/flow.svelte";
  import { droppedRepositories, openDropped } from "$lib/drop-open";
  import { connect } from "$lib/wiring";
  import { runDiskPass } from "$lib/disk-pass";
  import { DiskPasses } from "$lib/disk-refresh";
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
  import { withTracked } from "$lib/selected-refs";

  const measure = measurer((label, ms, detail) => void reportTiming(label, ms, detail));

  /** Maximising acts on the panel the pointer last entered; there is no focus ring yet. */
  let focused = $state<PanelId>("graph");

  $effect(() => {
    const landed = settle(focused, (panel) => layout.visible(panel));
    if (landed !== focused) focused = landed;
  });

  // Ctrl+A goes to the panel in focus (the one with the underlined header); behind a modal, to nothing.
  $effect(() => installSelectAll(window, () => (modals.any ? "modal" : focused)));
  let running = $state.raw<Map<number, string>>(new Map());
  /** Network operations whose git the footer's Cancel can stop, oldest first. */
  let networkOps = $state.raw<number[]>([]);
  const gitMissing = new GitMissingStore({
    probe: probeGit,
    candidates: findGitCandidates,
    apply: useGit,
    save: (path) => settings.set("gitPath", path),
  });
  let info = $state<AppInfo | null>(null);
  /** The last update check this session, for the line in About. */
  let lastUpdate = $state.raw<UpdateOutcome | null>(null);
  let opening = $state(false);
  let scanOpen = $state(false);
  let markedFiles = $state.raw<string[]>([]);
  /** The same ticks by the title of the list they are in: Unstaged, Staged. */
  let markedBySection = $state.raw<Record<string, string[]>>({});
  /** The rows of the list the Files panel shows, as it counts them itself. */
  let filesCount = $state<number | undefined>(undefined);
  type RepoMenuSubject =
    | { kind: "repository"; root: string; overview: import("$lib/ipc").RepoOverview | null }
    | {
        kind: "submodule";
        root: string;
        row: import("$lib/module-tree").ModuleRow;
        /** The repository it belongs to, when that is not the one the panels own. */
        top?: string;
      };
  let repoTarget = $state.raw<RepoMenuSubject | null>(null);
  let terminals = $state.raw<{ id: string; label: string }[]>([]);
  let groupTarget = $state<string | null>(null);
  /** Remembered per repository: a check command is worth typing once, not once per pause. */
  let checkCommand = $state("");
  let checkVerdict = $state.raw<import("$lib/ipc").HookRun | null>(null);
  let checking = $state(false);
  let markedRepos = $state.raw<string[]>([]);
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
  /** A commit the plan rewrites is on a remote: the editor warns about the force-push. */
  let rebasePublished = $state(false);
  let template = $state<string | null>(null);
  /** The commit Split Off is open on, as it was when it opened. */
  let split = $state.raw<import("$lib/split-off").SplitRequest | null>(null);
  /** Which shared branches hold a commit, once asked. Answering costs a graph walk per
      remote ref, so it is asked when the user opens a menu, not on every selection. */
  let protection = $state.raw<ReadonlyMap<string, readonly string[]>>(new Map());
  const protectedBy = $derived(protection.get(commit.oid ?? "") ?? []);
  /** Whether a commit is on the checked-out branch, once asked; same lifetime as `protection`. */
  let headness = $state.raw<ReadonlyMap<string, boolean>>(new Map());
  const onHead = $derived(headness.get(commit.oid ?? "") ?? true);

  /** Help ▸ Check for Updates, and the start-up check when the setting is on. The
      plugin is loaded on demand: nobody pays for the updater until it is wanted. */
  async function runUpdateCheck(quiet = false): Promise<void> {
    if (info?.portableDir) {
      if (!quiet) notices.inform("Check for Updates", PORTABLE_UPDATE_NOTE);
      return;
    }
    const [{ check }, { relaunch }] = await Promise.all([
      import("@tauri-apps/plugin-updater"),
      import("@tauri-apps/plugin-process"),
    ]);
    lastUpdate = await checkForUpdates(
      {
        check: () => check(),
        relaunch,
        confirm: (outcome) =>
          confirmation.ask({ title: "Check for Updates", message: message(outcome), confirm: "Install" }),
        report: (text) => notices.inform("Check for Updates", text),
        mayInstall: () => mayInstallUpdate(),
      },
      { quiet },
    );
  }

  async function learnProtection(oid: string): Promise<readonly string[]> {
    const id = repository.current?.repo;
    if (!id) return [];
    if (!headness.has(oid)) {
      void isAncestor(id, oid, "HEAD")
        .catch(() => true)
        .then((on) => (headness = new Map(headness).set(oid, on)));
    }
    const known = protection.get(oid);
    if (known) return known;
    const refs = await protectingRefs(id, oid).catch(() => [] as string[]);
    protection = new Map(protection).set(oid, refs);
    return refs;
  }
  /** Panels a disk event has outdated; cleared as each reload lands. */
  let stale = $state.raw<ReadonlySet<PanelId>>(new Set());
  let splitBusy = $state(false);
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
    conflicts.closeUnlessUnsaved();
  };

  $effect(() => {
    untrack(() => {
      startTracing();
      trace("startup", "the window is running");
      // Nothing in a Git client is a web page (R-127), nor has the browser's find bar.
      suppressNativeMenu(document);
      suppressBrowserFind(window);
      suppressBrowserNavigation(window);
      taskbar.start();
      const infoRead = getAppInfo()
        .then((result) => (info = result))
        .catch((err) => errors.report(err, "Could not read the application info"));
      void remoteOps.detectLfs();
      const settingsRead = settings.load().then(async () => {
        diff.whitespace = settings.current.ignoreWhitespace;
        // Only after the settings are read: the tick is what permits the network call.
        // And after the info: a portable build never checks.
        // Off the critical path: the probe has its own 5 s timeout and nothing waits for it.
        void gitMissing.check(settings.current.gitPath);
        await infoRead;
        if (settings.current.autoUpdate) void runUpdateCheck(true);
      });
      void settings.loadBindings();
      void health.loadIgnored();
      void toolbar.load();
      void networkDialog.load();
      void terminalChoices().then((found) => (terminals = found));
      const wanted = session.active;
      const remembered = wanted === null ? null : session.selected(wanted);
      void timed("startup", "restore the session", () => repository.restore()).then(async () => {
        const back =
          repository.openRepos.find((entry) => entry.root === wanted) ?? repository.openRepos[0];
        try {
          if (back) await activate(back.root, back.root === wanted ? remembered : null);
        } finally {
          // Only now is "nothing open" the answer rather than the first frame.
          repository.markReady();
        }
        // The open has settled (Open or Failed) and the setting is read: now it is known
        // whether the window would stay empty. Nothing waits on this; it only sets state.
        await settingsRead.catch(() => {});
        if (
          shouldShowAtStartup({
            enabled: settings.current.startupShowWelcome,
            restoring: false,
            openCount: repository.current ? 1 : 0,
            phase: repository.phase.kind,
          })
        ) {
          showWelcome();
        }
      });
    });
  });

  const fractions = $derived(layout.fractions);
  /** `--commit-panel-min` plus the splitter, in the same CSS pixels. */
  const COMMIT_MIN_PX = 126;
  let filesColumnHeight = $state(0);
  /** `--worktrees-panel-min` plus the splitter. */
  const WORKTREES_MIN_PX = 98;
  let leftColumnHeight = $state(0);
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
    repo ? stateBanner(repo.state, repo.indexLock, submodules.open !== null, loadedSubject) : null,
  );
  const tracked = $derived(repository.localBranches.find((b) => b.isHead));
  /** The staged rows the Files list shows; null until it has said. */
  let shownStaged = $state.raw<string[] | null>(null);
  const scope = $derived(commitScope(worktree.staged, fileMask, shownStaged));
  /** The subject of a commit the graph has loaded: the bisect banner names its commits. */
  function loadedSubject(oid: string): string | null {
    const at = graph.loadedIndexOf(oid);
    return at === null ? null : (graph.rowAt(at)?.commit.summary ?? null);
  }
  /** The subject of HEAD's commit titles the pull request; the graph has it loaded. */
  const headSummary = $derived.by(() => {
    const oid = repo && repo.head.kind !== "unborn" ? repo.head.oid : null;
    const at = graph.loadedIndexOf(oid);
    return at === null ? null : (graph.rowAt(at)?.commit.summary ?? null);
  });
  const prPlan = $derived(pullRequestFor({ remoteUrl: network.url, branch: tracked, title: headSummary }));
  const prUrl = $derived("url" in prPlan ? prPlan.url : null);
  const prReason = $derived("reason" in prPlan ? prPlan.reason : undefined);

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
  $effect(() => errors.report(diff.error, "Could not show the diff", diff.path ?? undefined));
  $effect(() => errors.report(graph.error, "Could not load the graph"));
  $effect(() => errors.report(hooks.error, hooks.failure ?? "Could not read the hooks"));
  $effect(() => errors.report(compareView.error, "Could not compare the commits"));
  $effect(() => errors.report(stashView.error, "Could not open the stash"));

  // Every change to the list — Drop in References, a pop, `git stash drop` in a terminal —
  // ends here, so the Files panel never lists a stash that is gone.
  $effect(() => {
    const entries = stashes.entries;
    untrack(() => {
      if (stashView.forgetIfGone(entries)) diff.clear();
    });
  });

  /** Refreshed with the rest of the state after every mutation, so the stack follows
      Continue and Abort; a reactive version fired on each loading toggle. */
  async function refreshProgress() {
    const id = repository.current?.repo;
    const epoch = repository.epoch;
    const found = id ? await rebaseProgress(id).catch(() => null) : null;
    if (repository.epoch === epoch) progress = found;
  }

  const mutation: MutationContext = {
    repo: () => repository.current?.repo ?? null,
    epoch: () => repository.epoch,
    report: (err) => errors.report(err, "Could not change the working tree"),
    loadWorktree: (id) => worktree.load(id),
    after: (paths) => afterMutation(paths),
  };
  diff.useMutation(mutation);

  /** See `runMutation`; `readsBack` for a worktree-store write that reads the list itself. */
  async function mutate(
    step: (repo: import("$lib/ipc").RepoId) => Promise<unknown>,
    paths: string[] = [],
    readsBack = false,
    repo?: import("$lib/ipc").RepoId,
  ): Promise<boolean> {
    return runMutation(mutation, step, paths, readsBack, repo);
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
    const conflicted = await repository.refreshStatus();
    const id = repository.current?.repo;
    await Promise.all([
      id ? stashes.refresh(id) : Promise.resolve(),
      id ? network.refresh(id) : Promise.resolve(),
      id ? recovery.refresh(id) : Promise.resolve(),
      // A checkout anywhere, in Branches or a terminal, moves which flow branch HEAD is on.
      id ? flow.refresh(id) : Promise.resolve(),
      submodules.refresh(),
      id ? conflicts.refresh(id, conflicted ?? undefined) : Promise.resolve(),
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
    ...splitMarked({
      marked: markedFiles,
      unstaged: worktree.unstaged,
      staged: worktree.staged,
      bySection: markedBySection,
    }),
    unstaged: worktree.unstaged.map((file) => file.path),
    staged: worktree.staged.map((file) => file.path),
    commit: commit.oid,
    head: headOid,
    branch: repo?.head.kind === "branch",
    upstream: Boolean(tracked?.upstream),
    merged: toolbar.merged,
    stashes: stashes.entries.length,
    undo: lastUndo !== null,
  });

  let networkActions = $state<ReturnType<typeof NetworkActions>>();
  const toolbarMenus = $derived<MenuContext>({
    remotes: network.remotes,
    current: networkActions?.currentPullRemote() ?? null,
    prefs: toolbar.prefs,
  });

  /** What Remote ▸ LFS ▸ Lock and Submodule act on: the ticked files, or the one in Diff. */
  const pickedFiles = $derived(markedFiles.length > 0 ? markedFiles : diff.path ? [diff.path] : []);
  const remoteActions = remoteOps.actions({
    files: () => pickedFiles,
    changed: afterRefChange,
    synchronize: () => void networkActions?.syncNow(),
    repoSettings: () => (repoSettingsOpen = true),
  });

  /** The palette shows each command's keys as the menu has them (11 §12, rule 5). */
  const menuKeys = $derived(effective(settings.bindings, settings.keymap));

  /** One action per command id: the palette, the native menu and the toolbar run these. */
  const actions: AppCommandActions = {
    open: () => void pickRepository(),
    clone: () => void openCloneWizard(),
    welcome: () => showWelcome(),
    fetch: () => void networkActions?.run("fetch"),
    pull: () => networkActions?.openPull(),
    "pull-defaults": () => networkActions?.pullNow(),
    push: () => networkActions?.openPush(),
    "push-defaults": () => networkActions?.run("push"),
    stash: stashAll,
    "stash-selection": () => stashSelected(),
    tag: () => void refActions?.addTag(null),
    stage: () => stage(targetsOf("stage", toolbarFacts)),
    unstage: () => unstage(targetsOf("unstage", toolbarFacts)),
    commit: () => void commitBox.commit(),
    "commit-amend": () => void commitBox.commit(true),
    "commit-window": () => {
      const current = repository.current;
      if (current) void openCommitWindow(current.repo, current.root);
    },
    "commit-message": () => void commitBox.focus(),
    undo: () => undo(),
    output: () => output.toggle(),
    "copy-path": () => void copyText(diff.path ?? ""),
    "copy-branch": () => void copyText(tracked?.name ?? ""),
    "copy-sha": () => void copyText(commit.oid ?? ""),
    "rebase-i": () => void openRebase(),
    "flow-init": () => void runFlow(() => flow.init(repo!.repo)),
    flowStart: (kind) => askFlowStart(kind),
    "flow-finish": () => askFlowFinish(),
    "split-off": () => void openSplit(),
    rollback: () => void rollbackFiles([]),
    close: () => void closeCurrent(),
    refresh: () => void repository.refresh(),
    branch: () => void runBannerAction("createBranch"),
    "reset-layout": () => layout.reset(),
    overlap: () => overlap.toggle(),
    avatars: () =>
      void settings.apply({
        ...settings.current,
        avatars: settings.current.avatars === "gravatar" ? "off" : "gravatar",
      }),
    hooks: () => {
      const id = repository.current?.repo;
      if (id) void hooks.show(id);
    },
    "perspective-main": () => layout.switch("main"),
    "perspective-review": () => layout.switch("review"),
    "maximize-panel": () => layout.toggleMaximized(focused),
    "worktree-add": () => worktreeActions?.openAdd(),
    "worktree-remove": () => {
      const entry = worktrees.current;
      if (removable(entry)) worktreeActions?.remove(entry);
    },
    "worktree-prune": () => void worktreeActions?.pruneAll(),
    "resolve-conflicts": () => openSolverAt(conflicts.path ?? conflicts.paths[0]),
    "undo-rewrite": () => void undoLastRewrite(),
    "range-diff": () => void showRangeDiff(),
    "maintenance-gc": () => void runNoticeAction({ id: RUN_GC, label: "Run gc…", targets: [] }),
    togglePanel: (panel) => layout.togglePanel(panel),
    palette: () => {
      if (commit.oid) void learnProtection(commit.oid);
      paletteOpen = true;
    },
    "check-updates": () => void runUpdateCheck(),
    "edit-config-repository": () => void openConfig("repository"),
    "edit-config-user": () => void openConfig("user"),
    exit: () => {
      // No window is being closed by hand, so the dialog must not say one is.
      exitFlow.fromCommand();
      // Closed in Rust (R-86): the webview is not allowed `window.close`, and the refusal
      // was silent — Exit from the menu, Alt+X and the palette did nothing (R-190).
      void closeThisWindow().catch(() => exitFlow.takeSource());
    },
    about: () => (aboutOpen = info !== null),
    settings: () => openSettings(),
    find: () => (finderOpen = true),
    pr: () => void openPullRequest(),
    scan: () => {
      scanOpen = true;
      void browseForScan();
    },
    journal: () => {
      journalOpen = true;
      void safety.refresh();
    },
    "fetch-all": () => void networkActions?.fetchAll(markedRepos),
    "reveal-log": () => void revealLog(),
    "copy-pr": () => {
      if (prUrl) void import("@tauri-apps/plugin-clipboard-manager").then((m) => m.writeText(prUrl));
    },
    blame: () => void showBlame(),
    abort: () => {
      const action = abortAction(banner);
      if (action) void runBannerAction(action);
    },
  };

  const palette = $derived<PaletteCommand[]>([
    ...appCommands(
      {
        context: commands,
        toolbar: toolbarFacts,
        lastUndo: lastUndo !== null,
        diffPath: diff.path,
        branch: tracked !== undefined,
        commitOid: commit.oid,
        flow: { initialised: flow.status.initialised, current: Boolean(flow.current) },
        protectedBy,
        onHead,
        worktrees: { removable: removable(worktrees.current), stale: hasStale(worktrees.entries) },
        conflicts: conflicts.paths.length,
        prReason,
        logPath: Boolean(info?.logPath),
        banner,
        openCount: repository.openRepos.length,
      },
      actions,
    ),
    ...bisectCommands(repo?.state, (action) => bisectActions?.command(action)),
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
        syncBlocked: reasonOf("sync", toolbarFacts),
      },
      remoteActions,
    ),
  ]);

  async function openPullRequest() {
    const url = prUrl;
    if (!url) return;
    if (tracked && needsPush(tracked)) {
      // Cancel opens nothing; Copy Pull Request Link is there for a form without the push.
      const push = await confirmation.ask({
        title: "Create Pull Request",
        message:
          tracked.upstream === null
            ? `${tracked.name} is not on the remote yet, so the form would have nothing to compare. Push it, then open the form?`
            : `${tracked.name} has ${tracked.ahead} commit(s) the remote has not seen. Push them, then open the form?`,
        confirm: "Push and Open",
      });
      if (!push) return;
      await networkActions?.run("push");
    }
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  }

  async function runFind(text: string) {
    await finder.run(repository.current?.repo ?? null, text).catch((err) => errors.report(err, "Could not search"));
  }

  function pickFound(item: import("$lib/ipc").Found) {
    finderOpen = false;
    const id = repository.current?.repo;
    if (!id) return;
    const step = foundStep(
      item,
      commit.oid !== null,
      (path) =>
        worktree.staged.some((file) => file.path === path) && !worktree.unstaged.some((file) => file.path === path),
    );
    if (step?.kind === "file") void openDiff(step.path);
    if (step?.kind === "worktree" || step?.kind === "staged") {
      // A stash in Files would sit beside a diff of the working tree.
      if (stashView.contents !== null) stashView.clear();
      void (step.kind === "staged" ? openStagedDiff(step.path) : openWorktreeDiff(step.path));
    }
    if (step?.kind === "reveal") {
      stashView.clear();
      void commit.select(id, step.oid);
      graph.requestReveal(step.oid);
    }
  }

  /** A command picked in the palette; one from the menu bar or its keys is not a recent
      command of the palette. */
  function runCommand(command: PaletteCommand, fromPalette = true) {
    paletteOpen = false;
    if (fromPalette) recentCommands = rememberCommand(recentCommands, command.id);
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
    // A dialog, the palette and Find Object close on their own Esc; nothing behind a modal
    // answers a key (11 §1).
    if (modals.any) return;

    // F6 walks the panels. Ctrl+Tab is left to the window: the menu agent owns the
    // accelerators, and browsers and hosts both claim that pair (issue 15). A diff with the
    // focus has already taken F6 for its next change, in the capture phase (11 §7).
    if (event.key === "F6") {
      if (event.defaultPrevented) return;
      event.preventDefault();
      focused = step(focused, (panel) => layout.visible(panel), event.shiftKey ? -1 : 1);
      return;
    }

    // Discard is Ctrl+Z in Files alone; everywhere else Ctrl+Z is the field's undo (11 §4).
    if (primary(event, ON_MAC) && !event.shiftKey && !event.altKey && event.code === "KeyZ") {
      if (focused === "files" && !typing(event)) {
        event.preventDefault();
        void discardFromToolbar(targetsOf("discard", toolbarFacts));
      }
      return;
    }
  }

  /** What Cancel goes back to. Taken when the dialog opens, not when it closes: by the
      time it closes, everything has already been applied and saved (R-122). */
  let settingsAtOpen = $state.raw<Settings | null>(null);
  let toolbarAtOpen: readonly string[] = [];
  let keymapAtOpen: import("$lib/keymap").Keymap = {};
  /** The page Preferences opens on: right-click on the toolbar lands on Toolbar. */
  let settingsStart = $state<string | undefined>(undefined);

  function openSettings(page?: string) {
    settingsAtOpen = { ...settings.current };
    toolbarAtOpen = toolbar.layout;
    keymapAtOpen = { ...settings.keymap };
    settingsStart = page;
    settingsOpen = true;
  }

  async function revertSettings() {
    const before = settingsAtOpen;
    settingsOpen = false;
    await toolbar.setLayout(toolbarAtOpen);
    await applySettings(before ?? settings.current, keymapAtOpen);
  }

  const preferencesHost: ApplyHost = {
    current: () => settings.current,
    apply: (next) => settings.apply(next),
    keymap: () => settings.keymap,
    setKeymap: (keymap) => settings.setKeymap(keymap),
    rebuiltMenu: () => pushMenuState(true),
    repo: () => repository.current?.repo ?? null,
    diff,
  };

  function applySettings(next: Settings, keymap: import("$lib/keymap").Keymap) {
    return applyPreferences(next, keymap, preferencesHost);
  }

  // The watcher is the only way Cogit learns about work done in a terminal alongside it.
  // One `git commit` arrives as four events, so they are collected and answered once, and
  // never while the answer to the last burst is still reading.
  const SETTLE_MS = 120;
  // A stream that never pauses for SETTLE_MS is still answered this often.
  const MAX_WAIT_MS = 1_000;
  const diskPasses = new DiskPasses(SETTLE_MS, MAX_WAIT_MS, (kinds, arrived) => applyDiskChanges(kinds, arrived));

  function onDiskChange(change: import("$lib/ipc").RepoChanged) {
    if (change.kind === "refs") {
      const moved = repository.openRepos.find((entry) => entry.repo.valueOf() === change.repo.valueOf());
      if (moved) repoPulse.refsMoved(moved.root);
    }
    const id = repository.current?.repo;
    if (!id || id.valueOf() !== change.repo.valueOf()) {
      const left = repository.openRepos.find((entry) => entry.repo.valueOf() === change.repo.valueOf());
      if (left) repoPulse.changed(left.root);
      return;
    }
    if (change.kind !== "hooks") stale = markStale(stale, change.kind);
    diskPasses.add(change.kind);
  }

  function onWatchLimited(event: import("$lib/ipc").WatchLimited) {
    notices.inform(
      "Working Tree Not Watched",
      `Changes made outside Cogit appear only after a refresh (F5). On Linux this is usually the inotify limit: raise fs.inotify.max_user_watches.

${event.error}`,
    );
  }

  async function applyDiskChanges(
    kinds: ReadonlySet<import("$lib/ipc").ChangeKind>,
    arrived: () => ReadonlySet<import("$lib/ipc").ChangeKind>,
  ) {
    await runDiskPass(
      {
        repo: () => repository.current?.repo ?? null,
        epoch: () => repository.epoch,
        commitSelected: () => commit.oid !== null,
        refreshRefs: () => repository.refreshRefs(),
        resetProtection: () => {
          protection = new Map();
          headness = new Map();
        },
        hooksOpen: () => hooks.open,
        refreshHooks: (id) => void hooks.refresh(id),
        loadWorktree: (id) => worktree.load(id),
        refreshWorktrees: (id) => void worktrees.refresh(id),
        afterMutation,
        refreshDiff: () => diff.refreshFromDisk(),
        reselectCommit: (id) => {
          if (commit.oid) void commit.select(id, commit.oid);
        },
        loadGraph: (id) => graph.load(id, graph.query),
        // What changed again while the pass read stays marked for the pass after it.
        freshened: (panels) => (stale = freshen(stale, panels, arrived())),
      },
      kinds,
    );
  }

  function filterGraph(query: import("$lib/ipc").CommitQuery) {
    const id = repository.current?.repo;
    if (!id) return;
    commit.clear();
    diff.clear();
    void graph.load(id, query);
  }

  async function stage(paths: string[]) {
    await mutate((id) => worktree.stage(id, paths), paths, true);
  }

  async function unstage(paths: string[]) {
    await mutate((id) => worktree.unstage(id, paths), paths, true);
  }

  async function ignore(paths: string[]) {
    await mutate((id) => addToGitignore(id, paths), paths);
  }

  async function ignoreLocally(paths: string[]) {
    await mutate((id) => addToExclude(id, paths), paths);
  }

  async function whyIgnored(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      const rules = await ignoreRules(id, paths);
      const lines = rules.map((rule) =>
        rule.source ? `${rule.path}: ${rule.source}:${rule.line}: ${rule.pattern}` : `${rule.path}: not ignored`,
      );
      notices.inform("Why Ignored", lines.join("\n"));
    } catch (err) {
      errors.report(err, "Could not check the ignore rules");
    }
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
    await mutate((repo) => worktree.discard(repo, paths), paths, true);
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
    if (go) await mutate((repo) => worktree.discard(repo, paths), paths, true);
  }

  /** To the Recycle Bin, from the menu and the list's own Delete button alike (#40). */
  async function deleteFromDisk(paths: string[]) {
    const id = repository.current?.repo;
    if (!id || paths.length === 0) return;
    const what = paths.length === 1 ? paths[0] : `${paths.length} files`;
    const bin = `the ${binName(OS)}`;
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
    // An empty list would commit every staged file, the hidden ones included.
    if (!id || scope.empty) return false;

    if (amend && (await publishedOrAssume(isPublished(id, "HEAD")))) {
      const go = await confirmation.ask({
        title: "Amend a Published Commit",
        message:
          "This commit is already on a remote. Amending it gives it a new id, so the branch " +
          "will need a force-push and anyone who pulled it will have to reset. Continue?",
        confirm: "Amend",
        warning: true,
      });
      if (!go) return false;
    }

    if (scope.empty) return false;
    if (scope.paths) {
      const confirmed = await confirmation.ask({
        title: "Commit What You See",
        message: `${
          scope.paths.length === 1
            ? `Commit only ${scope.paths[0]}?`
            : listedMessage(`Commit only these ${scope.paths.length} files?`, scope.paths)
        }\n\n${scope.warning}.`,
        confirm: "Commit",
        warning: true,
      });
      if (!confirmed) return false;
    }
    const epoch = repository.epoch;
    if (!noVerify) await announceHooks(id, "commit");
    try {
      await worktree.commit(id, message, amend, noVerify, scope.paths ?? []);
    } catch (err) {
      // A hook's refusal: the message stays in the box for another try.
      errors.report(err, "Could not commit");
      return false;
    } finally {
      runningHooks = undefined;
    }
    if (repository.epoch !== epoch) return true;
    diff.clear();
    await repository.refreshRefs();
    await afterMutation();
    void graph.load(id, graph.query);
    return true;
  }

  /** A fetch moves only remote-tracking refs: the refs and the graph are read again, the
      selected commit and the open diff stay. Which commits are published may have changed. */
  async function afterFetch(worked: RepoId) {
    const id = repository.current?.repo;
    if (!id || worked !== id) return;
    const epoch = repository.epoch;
    protection = new Map();
    headness = new Map();
    await repository.refreshRefs();
    if (repository.epoch !== epoch) return;
    await afterMutation();
    if (repository.epoch !== epoch) return;
    void graph.load(id, graph.query);
  }

  /** `worked` is the repository the change was made in; once the panels show another,
      reloading them would clear that one's selection and diff for nothing. */
  async function afterRefChange(worked?: RepoId) {
    const id = repository.current?.repo;
    if (!id || (worked !== undefined && worked !== id)) return;
    const epoch = repository.epoch;
    commit.clear();
    diff.clear();
    await repository.refreshRefs();
    if (repository.epoch !== epoch) return;
    void graph.load(id, graph.query);
    await worktree.load(id);
    if (repository.epoch !== epoch) return;
    await afterMutation();
  }

  const refTreeBase = $derived({
    head: repo?.head,
    branches: repo?.branches ?? [],
    tags: repo?.tags ?? [],
    stashes: stashes.entries,
    lost: recovery.lost,
    others: refs.others,
    showPseudoRefs: settings.current.refsShowPseudoRefs,
    remoteUrls: refs.urls,
    remotes: network.remotes,
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
    const ticked = settings.current.graphIncludeTracked
      ? withTracked(refs.visible, repo?.branches ?? [])
      : refs.visible;
    graph.visibleRefs = visibleTips(nodes, ticked);
    await graph.load(id, graph.query);
    watch.stop(`${graph.total} commits`);
  }

  // Include Tracked Remote Branches changes the tips the walk starts from (F-561).
  let trackedWalked = untrack(() => settings.current.graphIncludeTracked);
  $effect(() => {
    const on = settings.current.graphIncludeTracked;
    if (on === trackedWalked) return;
    trackedWalked = on;
    untrack(() => void reloadGraph());
  });

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
    void revealRef(node.oid, {
      inWalk: async (oid) => (await graph.indexOf(oid)) !== null,
      tickable: node.rev !== undefined && !node.disabled && !refs.visible.has(node.id),
      tick: async () => {
        refs.set(new Set([...refs.visible, node.id]));
        await reloadGraph();
      },
      reveal: (oid) => graph.requestReveal(oid),
    });
  }

  /** One side of a stash part against the commit it was taken from. */
  async function openStashDiff(part: "worktree" | "index" | "untracked", path: string) {
    const id = repo?.repo;
    const spec = stashView.spec(part);
    if (!id || !spec || !(await conflicts.leave()) || repo?.repo !== id) return;
    void diff.load(id, spec, path);
  }

  /** The merge on screen gives way to any other file, asking first if sides are picked. */
  async function openCompareDiff(path: string) {
    if (await conflicts.leave()) compareView.open(path);
  }

  /** A double click in Branches (item 40): HEAD does nothing, a folder folds in the tree. */
  function activateRef(node: RefNode) {
    const action = refActivation(node);
    if (action === "checkout") refActions?.checkOutNode(node);
    else if (action === "apply-stash") refActions?.applyStashNode(node);
    else if (action === "recover") {
      const found = recovery.lost.find((row) => row.oid === node.oid);
      if (found) void recoverCommit(found);
    }
  }

  /** The Checkout dialog's choice (item 40), with the question about a worktree that has
      the branch and the offer to stash what is in the way (`lib/checkout-flow.ts`). */
  function treeRev(target: import("$lib/ipc").CheckoutTarget): string {
    switch (target.kind) {
      case "branch":
        return target.name;
      case "commit":
        return target.oid;
      case "newBranch":
        return target.start;
      case "fastForward":
        return target.to;
    }
  }

  async function checkOut(request: CheckoutRequest) {
    const id = repository.current?.repo;
    if (!id) return;
    await runCheckout(request, {
      elsewhere: (branch) => heldElsewhere(id, branch),
      checkout: (target) => checkout(id, target),
      unportable: (target) => unportablePaths(id, treeRev(target)),
      confirm: (message) => confirmation.ask({ title: "Unsupported File Names", message, confirm: "Check Out", warning: true }),
      ask: (question) => autostashDialog.ask(question),
      autostash: (target, message, drop) => runSwitchWithAutostash(id, target, message, drop),
      report: (err, title) => errors.report(err, title),
      inform: (title, body) => notices.inform(title, body),
      after: () => afterRefChange(id),
    });
  }

  /** True when another worktree has the branch: the user was asked about that one instead. */
  async function heldElsewhere(id: RepoId, branch: string): Promise<boolean> {
    const elsewhere = await worktrees.holding(id, branch);
    if (!elsewhere || elsewhere.missing) return false;
    const go = await confirmation.ask({
      title: "Branch Is in Another Worktree",
      message: `${branch} is checked out in the worktree at ${elsewhere.path}. Switch to it?`,
      confirm: "Switch",
    });
    // A worktree of this repository opens in the panels, as a double-click in Worktrees
    // does; activating its folder made it a repository of its own in the list (R-184).
    const row = worktrees.entries.find((entry) => entry.path === elsewhere.path);
    if (go) await (row ? openWorktreeRow(row) : activate(elsewhere.path));
    return true;
  }

  async function undo() {
    const id = repository.current?.repo;
    if (!id) return;
    let undone;
    try {
      undone = await safety.undoShown(id);
    } catch (err) {
      errors.report(err, "Could not undo");
      return;
    }
    if (undone) await afterRefChange(id);
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
  /** The submodule whose open is under way, by key: the clicks of a double-click. */
  let moduleOpening: string | null = null;
  /** The same for a submodule of a repository the panels do not own, by root and key. */
  let foreignOpening: string | null = null;

  async function openModule(row: import("$lib/module-tree").ModuleRow) {
    const step = moduleClick(row.module.state, {
      key: row.key,
      // A worktree of the submodule opened from Worktrees leaves `open` as it was.
      shown: worktrees.ownerRoot === null ? submodules.open : null,
      opening: moduleOpening,
    });
    if (step === "offer") {
      await offerInitialise(row.key);
      return;
    }
    if (step === "stay") return;
    moduleOpening = row.key;
    try {
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
      await afterMutation();
    } finally {
      if (moduleOpening === row.key) moduleOpening = null;
    }
  }

  /** A submodule of a listed repository the panels do not own, open or closed (R-352): the
      repository is opened without taking the panels, its tree becomes the full one, and
      the submodule takes the panels. Clicking the repository afterwards comes back to it. */
  async function openForeignModule(root: string, row: import("$lib/module-tree").ModuleRow) {
    const target = `${root}\n${row.key}`;
    if (foreignOpening === target) return;
    foreignOpening = target;
    try {
      const epoch = repository.epoch;
      let owner: import("$lib/ipc").RepoSummary;
      try {
        owner = await openRepository(root);
      } catch (err) {
        errors.report(err, "Could not open the repository");
        return;
      }
      if (repository.epoch !== epoch) return;
      await submodules.own(owner.repo, owner.root);
      if (repository.epoch !== epoch) return;
      repository.keep(owner);
      await openModule(submodules.rows.find((each) => each.key === row.key) ?? row);
      void repository.refreshList();
    } finally {
      if (foreignOpening === target) foreignOpening = null;
    }
  }

  /** A closed row of Repositories: opened once, however many clicks arrive meanwhile. */
  function reopen(root: string) {
    const phase = repository.phase;
    if (reopenClick(root, phase.kind === "opening" ? phase.root : null) === "open") void activate(root);
  }

  /** Update of a submodule in the light tree of a repository the panels do not own: its
      repository opens into the list, the panels and the tree they own stay as they are. */
  async function updateForeignModule(top: string, row: import("$lib/module-tree").ModuleRow, init: boolean) {
    try {
      const owner = await openRepository(top);
      // A nested module is updated by the submodule above it.
      const parent = row.parent === "" ? owner.repo : (await openSubmodule(owner.repo, row.parent)).repo;
      await updateSubmodule(parent, row.path, init);
    } catch (err) {
      errors.report(err, "Could not update the submodule");
    }
    moduleForest.forget(top);
    void repository.refreshList();
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
    if (action.id === FIX_GIT) {
      gitMissing.reopen(gitReasonOf(notices.current?.body ?? ""));
      return;
    }
    const maintenance = MAINTENANCE[action.id];
    if (maintenance) {
      const id = repository.current?.repo;
      if (!id) return;
      const go = await confirmation.ask({ title: action.label.replace("…", ""), message: maintenance.question, confirm: "Run" });
      if (!go) return;
      try {
        await runMaintenance(id, maintenance.task);
      } catch (err) {
        errors.report(err, "Maintenance failed");
        await health.recheck();
        return;
      }
      successToast.show(maintenance.done);
      await health.recheck();
      // Fixed: the warning is gone with the recheck; if the recheck could not run, close it.
      if (notices.current?.action?.id === action.id) notices.dismiss();
      return;
    }
    if (action.id === TRUST_DIRECTORY) {
      const id = repository.current?.repo;
      if (!id) return;
      await trustDirectory(id).catch((err) => errors.report(err, "Could not trust the folder"));
      await health.recheck();
      return;
    }
    const owner = submodules.owner;
    if (action.id !== FETCH_MODULES || !owner) return;
    for (const key of action.targets) {
      try {
        const opened = await openSubmodule(owner, key);
        const remote = await trackedRemote(opened.repo);
        if (remote) await fetchRemote(opened.repo, remote, () => {});
      } catch (err) {
        errors.report(err, `Could not fetch in ${key}`);
      }
    }
    await health.recheck();
    await submodules.refresh();
  }

  /** Never changes the repository unasked: the answer is a question (R-149). */
  const initialiser = new ModuleInitialiser({
    ask: (key) =>
      confirmation.ask({
        title: "Submodule is not initialized",
        message: `Submodule ${key} is not initialized. Initialize and check it out now?`,
        confirm: "Initialize",
      }),
    update: async (key) => {
      const row = submodules.rows.find((entry) => entry.key === key);
      if (row) await refreshSubmodule(row);
    },
  });

  function offerInitialise(key: string) {
    if (!submodules.rows.some((entry) => entry.key === key)) return Promise.resolve();
    return initialiser.offer(key);
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
        source: refAt(oid, repo?.branches ?? [], repo?.tags ?? []),
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
      await rebaseOnto(id, { onto: refAt(oid, repo?.branches ?? [], repo?.tags ?? []), autostash: true });
    } catch (err) {
      errors.report(err, "Could not rebase");
    }
    await afterRefChange(id);
  }

  /** ORIG_HEAD is where the last merge, rebase or reset started: a different action from the
      journal's Undo, which reverses what Cogit itself recorded. */
  async function undoLastRewrite() {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      const question = undoRewriteQuestion(await undoRewriteInfo(id));
      if ("refused" in question) {
        await confirmation.ask({ title: "Undo Last Merge / Rebase / Reset", message: question.refused, confirm: "OK" });
        return;
      }
      const go = await confirmation.ask({
        title: "Undo Last Merge / Rebase / Reset",
        message: question.message,
        confirm: "Undo",
        warning: question.warning,
      });
      if (!go) return;
      await undoRewrite(id);
    } catch (err) {
      errors.report(err, "Could not undo the last merge, rebase or reset");
    }
    await afterRefChange(id);
  }

  /** git range-diff ORIG_HEAD...HEAD: each commit before a rebase or amend next to what it became (E1). */
  async function showRangeDiff() {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      output.show(await rangeDiff(id, "ORIG_HEAD", "HEAD"));
    } catch (err) {
      errors.report(err, "Could not compare before and after the rewrite");
    }
  }

  let runningHooks = $state<string | undefined>(undefined);

  /** Named in the footer while the step runs; git itself writes their output to the journal. */
  async function announceHooks(id: import("$lib/ipc").RepoId, stage: "commit" | "push") {
    const overview = await listHooks(id).catch(() => null);
    runningHooks = hooksNote(pendingHooks(overview, stage));
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

  /** Toolbar Apply Stash (#30). A conflicted apply still changed the working tree, so the
      panels reload either way; Git's output reaches the notification window. */
  async function applyNewestStash() {
    const id = repository.current?.repo;
    const newest = stashes.entries.find((entry) => entry.index === 0);
    if (!id || !newest) return;
    await stashes
      .apply(id, newest.oid, false)
      .catch((err) => errors.report(err, "Could not apply stash@{0}"));
    await afterRefChange(id);
  }

  async function runBannerAction(action: BannerAction) {
    if (bisectActions?.claims(action)) return bisectActions.banner(action);
    const id = repository.current?.repo;
    const state = repository.current?.state;
    if (!id || !state) return;
    const question = bannerQuestion(action, state);
    if (question && !(await confirmation.ask(question))) return;
    if (repository.current?.repo !== id) return;
    try {
      if (action === "abort") await abortOperation(id);
      if (action === "continue") await continueOperation(id);
      if (action === "skip") await skipOperation(id);
      if (action === "deleteLock") await removeIndexLock(id);
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

  /** Double-click opens the file in its own window, which survives a webview reload. A
      submodule has no lines to compare: it opens, as its row in Repositories does (R-537). */
  function openInWindow(path: string, spec = diff.spec) {
    const id = repository.current?.repo;
    // A conflicted file has three sides: it opens in the Conflict Solver, not in a diff.
    if (id && conflicts.paths.includes(path)) {
      openSolverAt(path);
      return;
    }
    if (!id || !spec) return;
    if (isModulePath(path, [worktree.unstaged, worktree.staged, commit.files, compareView.files])) {
      void openModuleAt(path);
      return;
    }
    void openCompareWindow(compareUrl(id, path, spec), diffWindowTitle(path)).catch((err) =>
      errors.report(err, "Could not open the file window"),
    );
  }

  /** The Conflict Solver for one file, a window of its own; `externalTool` also starts the
      merge tool in it. */
  function openSolverAt(path: string | undefined, externalTool = false) {
    const id = repository.current?.repo;
    if (!id || !path) return;
    void openSolverWindow(id, path, externalTool).catch((err) =>
      errors.report(err, "Could not open the Conflict Solver"),
    );
  }

  /** The submodule at `path` of the repository the panels show, opened in them. */
  async function openModuleAt(path: string) {
    const owner = submodules.owner;
    if (owner === null) return;
    const row = await findModuleRow(submodules.children, submodules.open, path, (parent) =>
      listSubmodules(owner, parent),
    ).catch((err) => {
      errors.report(err, "Could not open the submodule");
      return null;
    });
    if (row) await openModule(row);
  }

  // A compare window that was asked for a submodule hands it over and closes (R-537).
  $effect(() =>
    onOpenModule((request) => {
      if (repository.current?.repo.valueOf() === request.repo.valueOf()) void openModuleAt(request.path);
    }),
  );

  // A compare window staged, unstaged or discarded lines: read the lists again.
  $effect(() =>
    onTreeChange((change) => {
      if (repository.current?.repo.valueOf() === change.repo.valueOf()) {
        void afterWorkingTreeChange([change.path]);
      }
    }),
  );

  async function openDiff(path: string) {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (!id || !oid) return;
    const spec = { kind: "commitVsParent", oid } as const;
    if (diff.shows(spec, path)) return;
    if (!(await conflicts.leave()) || commit.oid !== oid) return;
    void diff.load(id, spec, path);
  }

  async function openStagedDiff(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    if (diff.shows({ kind: "indexVsHead" }, path)) return;
    if (!(await conflicts.leave()) || repository.current?.repo !== id) return;
    void diff.load(id, { kind: "indexVsHead" }, path);
  }

  async function openWorktreeDiff(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    // A conflicted file has three sides; a two-sided diff of it says nothing useful.
    if (conflicts.paths.includes(path)) {
      if (conflicts.path === path || !(await conflicts.leave())) return;
      if (repository.current?.repo !== id) return;
      diff.clear();
      void conflicts.open(id, path);
      return;
    }
    // A second click is not a toggle: it would fight the double-click that opens a window (#7).
    if (diff.shows({ kind: "workTreeVsIndex" }, path)) return;
    if (!(await conflicts.leave()) || repository.current?.repo !== id) return;
    const watch = measure("open-diff");
    void diff.load(id, { kind: "workTreeVsIndex" }, path).then(() => watch.stop(path));
  }

  /** Clearing everything the old repository put on screen — except the submodule tree,
      which belongs to the repository in the list and outlives the panels (R-129). */
  function forgetPanelsKeepingTheTree(keepWorktrees = false) {
    commit.clear();
    commitTree.clear();
    diff.clear();
    worktree.clear();
    stashes.clear();
    network.clear();
    recovery.clear();
    conflicts.clear();
    stashView.clear();
    flow.clear();
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
    const wasOpen = new Set(repository.openRepos.map((entry) => entry.root));
    if (!(await repository.open(root))) {
      trace(story, "activate: overtaken by a newer open, leaving the panels to it");
      return null;
    }
    const opened = repository.current;
    trace(story, `activate: repository.current is ${opened ? opened.name : "null"}`);
    if (opened) {
      refs.adopt(opened.root, buildRefTree({ ...refTreeInput, filter: "", collapsed: new Set() }));
      if (!wasOpen.has(opened.root)) moduleMemory.opened(opened.root);
      void submodules.own(opened.repo, opened.root);
      void health.check(opened.repo, opened.root, opened.name);
      session.setActive(opened.root);
      session.opened(opened.root);
      if (restoreOid) {
        void commit.select(opened.repo, restoreOid);
        // Waits for the graph to reach it: the list opens at the top otherwise.
        graph.requestReveal(restoreOid);
      }
      void timed(story, "graph", () => reloadGraph());
      void refs.loadUrls(opened.repo);
      void worktrees.refresh(opened.repo);
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

  /** What the panels show, as a row of Repositories sees it. */
  function panelsNow(): import("$lib/repo-click").RepoClickState {
    const phase = repository.phase;
    return {
      shown: repository.current?.repo ?? null,
      opening: phase.kind === "opening" ? phase.root : null,
      moduleOwner: submodules.open !== null ? submodules.owner : null,
      worktreeOwner: worktrees.ownerRoot,
    };
  }

  /** A click in the Repositories list (#50): the one on screen reloads nothing, the owner
      of the submodule or worktree on screen comes back from what was kept, keeping its
      submodule tree, and only another repository goes through a full open. */
  async function selectRepository(entry: import("$lib/ipc").RepoOverview) {
    const step = repoClick(entry, panelsNow());
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
    await afterMutation();
  }

  /** Restores a past version into the working tree; HEAD stays where it is. */
  async function rollbackFiles(paths: string[]) {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    const what = paths.length > 0 ? paths.join(", ") : "every file";
    const go = await confirmation.ask({
      title: "Roll Back",
      message: `Restore ${what} as it was in ${shortOid(rev)}? The changes in the working tree are stashed first, so Undo can bring them back.`,
      confirm: "Roll Back",
      warning: true,
    });
    if (!go) return;
    await mutate((repo) => rollbackTo(repo, rev, paths), paths);
  }

  /** A drop never acts on its own: the user picks from the menu it opens. */
  function onBranchDrop(sourceName: string, target: Branch, x: number, y: number) {
    const source = repository.localBranches.find((b) => b.name === sourceName);
    if (!source) return;
    const canFastForward = target.isHead && source.oid !== target.oid;
    const payload = { kind: "branch" as const, id: sourceName };
    const onto = { kind: "branch" as const, id: target.name };
    const head = repository.localBranches.find((b) => b.isHead)?.name ?? null;
    const actions = dropActions(payload, onto, canFastForward, head);
    if (actions.length === 0) return;
    dropMenu = { actions, source: payload, target: onto, x, y };
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
      const go = await confirmation.ask({
        title: "Rewrite History",
        message: `${action.title}. This rewrites history. Continue?`,
        confirm: "Rewrite",
        warning: true,
      });
      if (!go) return;
    }

    const revision = (name: string) => localRevision(name, repo?.branches ?? [], repo?.tags ?? []);
    try {
      if (action.id === "merge") {
        await mergeInto(id, {
          source: revision(menu.source.id),
          noFastForward: false,
          squash: false,
          message: null,
        });
      } else if (action.id === "rebase") {
        await rebaseOnto(id, { onto: revision(menu.target.id), autostash: true });
      } else if (action.id === "fastForward") {
        await mergeInto(id, {
          source: revision(menu.source.id),
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

  function onCommitDrop(sourceOid: string, targetOid: string, x: number, y: number) {
    const source = { kind: "commit" as const, id: sourceOid };
    const target = { kind: "commit" as const, id: targetOid };
    const actions = dropActions(source, target, false);
    if (actions.length === 0) return;
    dropMenu = { actions, source, target, x, y };
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
    rebasePublished = await planPublished(plan, (oid) => publishedOrAssume(isPublished(id, oid)));
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
    const plan = rebasePlan;
    rebasePublished = await planPublished(plan, (oid) => publishedOrAssume(isPublished(id, oid)));
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
  let refGroupActions = $state<ReturnType<typeof RefGroupActions>>();
  let selectionActions = $state<ReturnType<typeof SelectionActions>>();
  let bisectActions = $state<ReturnType<typeof BisectActions>>();
  let worktreeActions = $state<ReturnType<typeof WorktreeActions>>();
  let fileTarget = $state.raw<FileScope | null>(null);
  let fileSection = $state<"worktree" | "index" | "commit">("worktree");
  let aboutOpen = $state(false);
  let configEdit = $state.raw<{
    scope: import("$lib/ipc").ConfigScope;
    /** The repository the text was read from; saving writes there, not to the shown one. */
    repo: import("$lib/ipc").RepoId | null;
    file: import("$lib/ipc").ConfigFile;
    problem: { line: number | null; message: string } | null;
    saving: boolean;
  } | null>(null);

  async function openConfig(scope: import("$lib/ipc").ConfigScope) {
    const id = scope === "repository" ? (repository.current?.repo ?? null) : null;
    if (scope === "repository" && id === null) return;
    const epoch = repository.epoch;
    try {
      const file = await readGitConfig(id, scope);
      // Left for another repository while reading: the dialog would show a stale file.
      if (scope === "repository" && repository.epoch !== epoch) return;
      configEdit = { scope, repo: id, file, problem: null, saving: false };
    } catch (err) {
      errors.report(err, "Could not read the Git config");
    }
  }

  /** Git reads the text back before it is written; a refusal keeps the dialog open on
      the line it named. The config changes remotes and core.*, so the rest is re-read. */
  async function saveConfig(text: string) {
    const edit = configEdit;
    if (!edit) return;
    configEdit = { ...edit, saving: true };
    try {
      await writeGitConfig(edit.repo, edit.scope, text, edit.file.crlf);
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
    if (repository.current && (edit.repo === null || edit.repo === repository.current.repo)) {
      await repository.refresh();
      await afterMutation();
    }
  }

  /** Right-clicking a ticked row acts on the whole tick of its list; right-clicking any
      other row acts on that one, which is what every file manager does. */
  function fileScope(path: string, section?: string): string[] {
    const ticked = section === undefined ? markedFiles : (markedBySection[section] ?? []);
    return ticked.includes(path) ? [...ticked] : [path];
  }

  void desktop.load();

  /** `section` is the list the row sits in: "Staged" is the index, the rest the working
      tree (#40). A commit's files get their own menu (#41). */
  async function fileContext(
    path: string,
    event: MouseEvent,
    section?: string,
    rows: readonly import("$lib/ipc").FileEntry[] = [],
  ) {
    const id = repository.current?.repo;
    if (!id) return;
    const { clientX: x, clientY: y } = event;
    const paths = fileScope(path, section);
    const info = desktop.info;

    if (!onWorkingTree) {
      const rowOf = (each: string) => shownRow(each, rows, commit.files);
      const statuses = paths.map((each) => rowOf(each)?.status ?? "modified");
      const clicked = rowOf(path);
      fileTarget = { path, paths, statuses, rev: commit.oid, oldPath: clicked?.oldPath ?? null };
      fileSection = "commit";
      const items = commitFileMenu({
        status: clicked?.status ?? "modified",
        count: paths.length,
        onDisk: true,
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
    solver: (path) => openSolverAt(path),
    externalTool: (path) => openSolverAt(path, true),
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
    ignoreLocally: (paths) => void ignoreLocally(paths),
    whyIgnored: (paths) => void whyIgnored(paths),
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
    /** The repository the sides were read from; saving writes there. */
    repo: import("$lib/ipc").RepoId;
    sides: import("$lib/ipc/file-menus").IndexEditorSides;
    saving: boolean;
  } | null>(null);

  async function openIndexEditor(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    const epoch = repository.epoch;
    try {
      const sides = await fileMenus.indexEditorSides(id, path);
      if (repository.epoch !== epoch) return;
      indexEditing = { path, repo: id, sides, saving: false };
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
      false,
      open.repo,
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
      repoSettingsOpen = false;
      rebaseOpen = false;
      split = null;
      finderOpen = false;
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
    if (refGroupActions?.claims(node)) {
      await refGroupActions.context(node, x, y);
      return;
    }
    const items = refMenu({ kind: node.kind });
    if (items.length === 0) return;
    refTarget = node;
    await popupContextMenu(items, x, y).catch(() => {});
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
    if (!id) return;
    const branch = await flow.headNow(id);
    if (repository.current?.repo !== id) return;
    if (!branch) {
      errors.message("HEAD is not on a Git-Flow branch.", "Could not finish the branch");
      return;
    }
    if (branch.kind === "feature") {
      const develop = flow.status.config.develop;
      const go = await confirmation.ask({
        title: `Finish ${branch.full}`,
        message: `Merge ${branch.full} into ${develop} and delete it?`,
        confirm: "Finish",
        warning: true,
      });
      if (go) await runFlow(() => flow.finish(id, branch.kind, branch.name, null));
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
      void confirmation
        .ask({
          title: "Delete Group",
          message: removalQuestion(repoGroups.groups, target),
          confirm: "Delete",
          warning: true,
        })
        .then((yes) => {
          if (yes) repoGroups.remove(target);
        });
      return true;
    }
    return false;
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
    await afterMutation();
  }

  /** A row of the Repositories list, open or closed (#36). */
  function repoContext(row: ListedRepo, x: number, y: number) {
    return repoMenuRow.hold(row.root, () => showRepoMenu(row, x, y));
  }

  async function showRepoMenu(row: ListedRepo, x: number, y: number) {
    const info = desktop.info;
    repoTarget = { kind: "repository", root: row.root, overview: row.overview };
    const active = row.overview !== null && repo?.repo.valueOf() === row.overview.repo.valueOf();
    // What the row itself shows: a closed row knows it is missing only from its pulse.
    const { missing } = rowSync({
      overview: row.overview,
      owned: active,
      pulse: repoPulse.pulses.get(row.root),
      fetchFailed: false,
    });
    const items = repoMenu(
      {
        kind: "repository",
        active,
        open: row.overview !== null,
        missing,
        pinned: row.pinned,
        group: repoGroups.groups.of[row.root] ?? UNGROUPED,
        groups: groupChoices(repoGroups.groups),
      },
      info,
    );
    await popupContextMenu(items, x, y).catch(() => {});
  }

  /** A submodule node: the same menu, with the four list-only items explained away. */
  async function moduleContext(
    row: import("$lib/module-tree").ModuleRow,
    x: number,
    y: number,
    foreign?: string,
  ) {
    const top = foreign ?? submodules.ownerRoot;
    if (!top) return;
    await repoMenuRow.hold(moduleRoot(top, row.key), () => showModuleMenu(row, x, y, top, foreign));
  }

  async function showModuleMenu(
    row: import("$lib/module-tree").ModuleRow,
    x: number,
    y: number,
    top: string,
    foreign?: string,
  ) {
    const info = desktop.info;
    const open = foreign === undefined && submodules.open === row.key;
    repoTarget = { kind: "submodule", root: `${top}/${row.key}`, row, top: foreign };
    const items = repoMenu(
      {
        kind: "submodule",
        active: open,
        open,
        missing: false,
        pinned: false,
        group: UNGROUPED,
        groups: [],
        module: row.module,
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
        .ask({ title: "New Group", label: "Name", confirm: "Create", validate: textProblem })
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
        if (target.kind === "submodule" && target.top) void openForeignModule(target.top, target.row);
        else if (target.kind === "submodule") void openModule(target.row);
        else if (target.overview) void selectRepository(target.overview);
        // Like a click on the closed row: activate takes it off the closed list only once it
        // opened, so a folder that moved keeps its row.
        else reopen(root);
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
        void networkActions?.syncListed(target, command.id === "repo-pull" ? "pull" : "push");
        return true;
      case "repo-update":
        if (target.kind === "submodule") {
          const { row, top } = target;
          void updateModule(row.module, {
            ask: (request) => confirmation.ask(request),
            update: (init) => (top ? updateForeignModule(top, row, init) : refreshSubmodule(row)),
          });
        }
        return true;
      case "repo-pin":
        repoList.togglePin(root);
        return true;
      case "repo-rename":
        if (target.kind === "repository") void renameListed(root, target.overview);
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

  async function backToModuleOwner() {
    const owner = repository.openRepos.find((entry) => entry.root === submodules.ownerRoot);
    if (owner) await selectRepository(owner);
  }

  /** The row stays in the list, closed; Remove is what takes it out. A submodule closes
      back to the repository it belongs to. */
  async function closeListed(target: RepoMenuSubject) {
    if (target.kind === "submodule") {
      await backToModuleOwner();
      return;
    }
    const { overview } = target;
    if (!overview) return;
    repoList.closed(target.root);
    moduleMemory.setOpen(target.root, false);
    const wasActive = holdsPanels(overview, panelsNow());
    const last = wasActive && repository.openRepos.every((entry) => entry.repo === overview.repo);
    if (wasActive) {
      commit.clear();
      diff.clear();
      health.clear();
    }
    // Its submodule or worktree on screen goes with it: left there, the panels and the
    // submodule tree went on sending commands to the closed repository.
    if (wasActive && !isActive(overview)) {
      forgetPanels();
      worktrees.ownerRoot = null;
      repository.close();
    }
    // In the frame the other panels empty in, not a round trip after them.
    if (last) graph.clear();
    await repository.closeOne(overview.repo);
    if (!wasActive) return;
    const next = repository.openRepos[0];
    if (next) await activate(next.root);
    else if (!last) graph.clear();
  }

  async function renameListed(root: string, overview: import("$lib/ipc").RepoOverview | null) {
    const name = await prompt.ask({
      title: "Rename",
      label: "Name shown in the list; the folder keeps its name",
      value: listedName(repoList.list, root, overview),
      confirm: "Rename",
      validate: textProblem,
    });
    if (name !== null) repoList.rename(root, name);
  }

  async function removeListed(target: RepoMenuSubject) {
    if (target.kind !== "repository") return;
    const name = listedName(repoList.list, target.root, target.overview);
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
    moduleMemory.forget(target.root);
    repoPulse.forget(target.root);
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
      case "lost-copy-sha":
        if (node.oid) void copyText(node.oid);
        return true;
      case "lost-toggle":
        void refGroupActions?.toggle(node);
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
    const epoch = repository.epoch;
    let request;
    try {
      request = await splitRequest(rev, {
        files: async (oid) =>
          (commit.oid === oid && !commit.loading ? commit.files : await readCommitFiles(id, oid)).map(
            (file) => file.path,
          ),
        published: (oid) => publishedOrAssume(isPublished(id, oid)),
      });
    } catch (err) {
      errors.report(err, "Could not read the commit");
      return;
    }
    if (repository.epoch === epoch) split = request;
  }

  async function runSplit(paths: string[], message: string, splitFirst: boolean) {
    const id = repository.current?.repo;
    const rev = split?.oid;
    if (!id || !rev) return;
    splitBusy = true;
    try {
      await splitOff(id, rev, paths, message, splitFirst);
      split = null;
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
        await openRepository(root)
          .then((opened) => moduleMemory.opened(opened.root))
          .catch((err) => errors.report(err, "Could not open the repository"));
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
      errors.report(err, "Could not open the third-party licenses"),
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

  function openCloneWizard() {
    return cloneWizard.start(parentFolder(repository.current?.root ?? session.recent[0] ?? ""));
  }

  /** The Welcome dialog (F-586): rows come from the same recent list the app keeps. */
  const welcomeRows = $derived(mruRows(session.recent));
  let welcomeTarget: string | null = null;

  function showWelcome() {
    welcome.show(welcomeRows.map((row) => row.path));
  }

  /** Every spelling of the path goes: the list shows one row for all of them. */
  function forgetWelcomePath(path: string) {
    const key = pathKey(path);
    for (const spelling of session.recent.filter((entry) => pathKey(entry) === key)) {
      session.forgetRecent(spelling);
    }
    welcome.listChanged(welcomeRows.length);
  }

  function forgetWelcomeTarget() {
    const path = welcomeTarget;
    welcomeTarget = null;
    if (path !== null && welcome.open) forgetWelcomePath(path);
  }

  async function welcomeContext(path: string, x: number, y: number) {
    welcomeTarget = path;
    await popupContextMenu([menuItem(WELCOME_FORGET, "Remove from List", true, "Delete")], x, y).catch(() => {});
  }

  async function openFromWelcome(root: string) {
    welcome.close();
    opening = true;
    try {
      await activate(root);
    } finally {
      opening = false;
    }
  }

  async function runWelcome(action: WelcomeAction) {
    if (action.kind === "open") return openFromWelcome(action.path);
    // Clone opens over the Welcome dialog: Cancel comes back to it, Finish closes it.
    if (action.kind === "clone") return openCloneWizard();

    const picked = await pickFolder("Add or Create Repository");
    if (picked === null) return;
    const kind = await folderKind(picked).catch((err) => {
      errors.report(err, "Could not read the folder");
      return null;
    });
    if (kind === null) return;
    const plan = folderPlan(kind);
    if (plan === "gone") {
      notices.inform("Add or Create Repository", `${picked} is not a folder.`);
      return;
    }
    let root = picked;
    if (plan === "init") {
      const go = await confirmation.ask({
        title: "Initialize Repository",
        message: `${picked} is not a Git repository. Run git init there and open it?`,
        confirm: "Initialize",
      });
      if (!go) return;
      try {
        root = await initRepository(picked);
      } catch (err) {
        errors.report(err, "Could not initialize the repository");
        return;
      }
    }
    await openFromWelcome(root);
  }

  /** Repository ▸ Clone…'s Finish: the clone runs in the footer, then opens as Open would. */
  async function startClone(request: CloneRequest) {
    const login = cloneWizard.login;
    cloneWizard.close();
    welcome.close();
    await runClone(request, {
      run: (operation) => network.run(null, "Cloning", operation),
      clone: (wanted, onLine) => cloneRepository(wanted, login, onLine),
      open: async (root) => {
        opening = true;
        try {
          await activate(root);
        } finally {
          opening = false;
        }
      },
      report: (err, title) => errors.report(err, title),
    });
  }

  async function pickFolder(title: string): Promise<string | null> {
    const picked = await openFolderDialog({ directory: true, title });
    return typeof picked === "string" ? picked : null;
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

  /** Ctrl+W: Close Repository of the row the panels show, which its menu names with the
      same chord (F-361). */
  async function closeCurrent() {
    const step = closeStep(panelsNow());
    if (step.kind === "worktree") {
      const owner = repository.openRepos.find((entry) => entry.root === step.owner);
      await (owner ? selectRepository(owner) : comeBack(step.owner));
    } else if (step.kind === "module") {
      await backToModuleOwner();
    } else if (step.kind === "repository") {
      const listed = repository.openRepos.find((entry) => entry.repo === step.repo);
      if (listed) {
        await closeListed({ kind: "repository", root: listed.root, overview: listed });
        return;
      }
      commit.clear();
      diff.clear();
      health.clear();
      await repository.closeOne(step.repo);
    }
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
      nobody wrote yet, text typed in a dialog. Everything else is already on disk or in the
      draft store. */
  function unsavedWork(): string | null {
    return unsavedSummary({
      hook: hooks.dirty ? hooks.editing : null,
      merge: conflicts.regions.length > 0 ? conflicts.path : null,
      // The Hooks dialog is dirty for the same hook, which is named already.
      dialogs: modals.unsaved.filter((title) => !(hooks.dirty && title === "Hooks")),
    });
  }

  /** The footer's Cancel. `false` back means it ended or never reached git meanwhile. */
  function cancelNetworkOperation(): void {
    const id = cancellable(networkOps);
    if (id === null) return;
    void cancelNetwork(id).catch((err) => errors.report(err, "Could not cancel the operation"));
  }

  async function mayClose(): Promise<boolean> {
    const source = exitFlow.takeSource();
    flushTrace();
    session.persist();
    const what = unsavedWork();
    const question = { title: "Unsaved Work", message: `${what} Close anyway?`, confirm: "Close", warning: true };
    if (what && !(await confirmation.ask(question))) return false;
    // Someone who just said "close anyway" has been asked once already.
    const confirm = what ? false : settings.current.confirmExit;
    return exitFlow.ask(source, confirm, listOperations);
  }

  /** Installing an update restarts the app, which loses the same things closing does and
      stops whatever git runs. The plugin exits on its own on Windows, past `RunEvent::Exit`. */
  async function mayInstallUpdate(): Promise<boolean> {
    const what = unsavedWork();
    const question = { title: "Unsaved Work", message: `${what} Restart anyway?`, confirm: "Restart", warning: true };
    if (what && !(await confirmation.ask(question))) return false;
    flushTrace();
    session.persist();
    // Asks only while operations run: "Exit When Done" installs once they are over.
    return exitFlow.ask("command", false, listOperations);
  }

  /** No close request is pending here, so the window is destroyed rather than closed:
      closing would ask the same question a second time. */
  async function onSessionEnding() {
    flushTrace();
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
      void openDropped(droppedRepositories(event.paths), {
        openInList: openRepository,
        activate,
        refreshList: () => repository.refreshList(),
        report: (err, title) => errors.report(err, title),
      });
    }
  }

  // The rows of Repositories are read in the background, never while the repository on
  // screen is busy (R-353).
  repoPulse.setBusy(
    () =>
      running.size > 0 ||
      repository.busy ||
      network.running !== null ||
      networkActions?.bulkProgress() !== undefined ||
      graph.loading,
  );
  $effect(() =>
    repoPulse.setOwned(
      shownRowRoot({
        current: repository.current?.root ?? null,
        moduleOwnerRoot: submodules.ownerRoot,
        openModule: submodules.open,
        worktreeOwnerRoot: worktrees.ownerRoot,
      }),
      repository.current ? summaryPulse(repository.current) : null,
    ),
  );

  /** The other worktrees' folders have no watcher: their marks are read again when the
      window comes back and once a minute while there are any. */
  const worktreesUnwatched = $derived(othersToWatch(worktrees.entries));
  function revisitWorktrees() {
    const id = repository.current?.repo;
    if (id && worktreesUnwatched) void worktrees.refresh(id);
  }
  $effect(() => {
    if (!worktreesUnwatched) return;
    const timer = setInterval(revisitWorktrees, 60_000);
    return () => clearInterval(timer);
  });
  $effect(() => repoPulse.fetchEvery(settings.current.backgroundFetchMinutes));
  // Only the repository on screen is watched; the rest are the pulse's (R-351).
  const shownRepository = new ShownRepository(showRepository);
  $effect(() => shownRepository.set(repository.current?.repo ?? null));

  /** One subscription for everything the window hears from outside itself. */
  $effect(() =>
    connect({
      repoChanged: onDiskChange,
      watchLimited: onWatchLimited,
      operationChanged: (event) => {
        running = applyOperation(running, event);
        networkOps = trackCancellable(networkOps, event);
        exitFlow.observe(event);
      },
      avatarReady: (email) => void avatars.refresh(email),
      mergeResolved: (event) =>
        void answerMergeResolved(event, {
          shown: () => repository.current?.repo ?? null,
          resolvedElsewhere: (path) => conflicts.resolvedElsewhere(path),
          reload: () => afterWorkingTreeChange(),
        }),
      revealCommit: (event) => {
        if (repository.current?.repo.valueOf() !== event.repo.valueOf()) return;
        stashView.clear();
        void commit.select(event.repo, event.oid);
        graph.requestReveal(event.oid);
      },
      commandRecorded: (event) => void output.notice(event),
      closeRequested: mayClose,
      sessionEnding: () => void onSessionEnding(),
      dragDrop: onDragDrop,
    }),
  );

  /** The Errors window's Show conflicts: the first conflicted file of that repository opens
      in the files panel's three-sided preview. */
  async function showConflictsOf(root: string) {
    const current = repository.current;
    if (!current || current.root !== root) return;
    await conflicts.refresh(current.repo);
    const first = conflicts.paths[0];
    if (first) await openWorktreeDiff(first);
  }

  // The Errors window sends its choices here; failed commands anywhere reach its queue.
  $effect(() => {
    const pending = errorWindow.own((root) => void showConflictsOf(root));
    return () => void pending.then((stop) => stop());
  });

  // A warning about conflicts goes with the conflicts.
  $effect(() => {
    const root = repository.current?.root;
    const none = conflicts.paths.length === 0;
    if (root && none) untrack(() => errorWindow.conflictsResolved(root));
  });

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
      void networkActions?.run(kind);
    };
  }


  $effect(() => {
    const pending = onMenuCommand((id) => {
      pushMenuState(true);
      // The Welcome dialog is a modal, so its own menu answers before the modal check.
      if (id === WELCOME_FORGET) return forgetWelcomeTarget();
      if (!menuCommandRuns(id, modals)) return;
      if (id === "toolbar-preferences") return openSettings("toolbar");
      if (id === "select-all") return selectAllFromMenu();
      if (refActions?.run(id)) return;
      if (refGroupActions?.run(id)) return;
      if (selectionActions?.run(id)) return;
      if (bisectActions?.run(id)) return;
      if (runGroupCommand(id)) return;
      if (runRepoCommand(id)) return;
      if (worktreeActions?.run(id)) return;
      if (runFileCommand(id)) return;
      if (runRefCommand(id)) return;
      const command = palette.find((entry) => entry.id === id);
      if (command && !command.unavailable) runCommand(command, false);
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

  // The bar the page draws reads the same state the native one is given.
  const sendMenuState = menuStatePusher((disabled, checked) => {
    webMenus.pushState(disabled, checked);
    return setMenuState(disabled, checked);
  });

  /** A rebuilt bar starts with every tick cleared, so this runs again after a keymap save;
      and after a menu command, since muda flips a clicked tick on its own. */
  function pushMenuState(resend = false) {
    const checked = checkedIds({
      panels: PANELS.filter((panel) => layout.visible(panel)),
      output: output.open,
      maximized: layout.maximized !== null,
      overlap: overlap.enabled,
      avatars: avatars.enabled,
      perspective: layout.active,
    });
    sendMenuState(disabledIds(palette), checked, { resend });
  }

  // The native menu is not reactive, so the derived state is pushed to it. muda flips a
  // tick itself when the item is clicked, so this also puts the wrong one back.
  $effect(pushMenuState);
</script>

<!-- A closed repository has no watcher: its row is read again when the user comes back. -->
<svelte:window
  {onkeydown}
  onfocus={() => {
    repoPulse.revisit();
    revisitWorktrees();
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
          undo: actions.undo,
          stash: actions.stash,
          "stash-selection": actions["stash-selection"],
          "quick-stash-all": () => quickStashAll(),
          "quick-stash-selection": () => stashSelected(false),
          "apply-stash": () => applyNewestStash(),
          tag: actions.tag,
          "push-to": () => refActions?.pushToCurrent(),
          pull: actions.pull,
          push: actions.push,
          "pull-defaults": actions["pull-defaults"],
          "push-defaults": actions["push-defaults"],
          sync: () => networkActions?.syncNow(),
          "sync-order": (order) =>
            networkActions?.syncNow(order === "pushThenPull" ? "pushThenPull" : "pullThenPush"),
          "fetch-remote": (remote) => networkActions?.fetchRemotes(remote ? [remote] : []),
          "fetch-remotes": () => networkActions?.fetchRemotes(network.remotes),
          "pull-scope": (scope) =>
            void toolbar.set("pullScope", scope === "all" ? "all" : "current"),
          "delete-merged": () =>
            void toolbar.set("deleteMergedAfterPull", !toolbar.prefs.deleteMergedAfterPull),
          stage: actions.stage,
          unstage: actions.unstage,
          discard: () => void discardFromToolbar(targetsOf("discard", toolbarFacts)),
          merge: () => mergeSelected(),
          rebase: () => rebaseSelected(),
          "rebase-i": actions["rebase-i"],
        }
      : {}}
  />

  {#if banner && !shown.graph}
    <StateBanner {banner} busy={repository.busy} onaction={runBannerAction} />
  {/if}

  <div class="workspace" bind:clientWidth={workspaceWidth}>
    {#if emptyStateVisible(repository.phase, repository.ready)}
      <StartScreen
        onopen={() => void pickRepository()}
        onclone={() => void openCloneWizard()}
        onwelcome={() => showWelcome()}
      />
    {/if}
    {#if leftColumn}
    <div
      class="left-column"
      bind:clientHeight={leftColumnHeight}
      style:flex={shown.diff || topRow ? `0 1 ${fractions.leftColumn * 100}%` : "1 1 auto"}
    >
      {#if reposColumn}
      <div
        class="repos-column"
        class:grow={!shown.refs}
        style:flex={shown.refs ? `0 0 ${fractions.repositories * 100}%` : undefined}
      >
      {#if shown.repositories}
      <div
        class="pane grow"
        role="region"
        aria-label={PANEL_TITLES.repositories}
        onpointerdown={() => (focused = "repositories")}
      >
        <Panel
          title="Repositories"
          active={focused === "repositories"}
          count={listedRepos(repository.openRepos, repoList.list).length}
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
            oncontext={(row, x, y) => void repoContext(row, x, y)}
            onreopen={reopen}
            onmarked={(roots) => (markedRepos = roots)}
            onaddgroup={askAddGroup}
            ongroupcontext={(id, x, y) => void groupContext(id, x, y)}
            onopenmodule={(row) => void openModule(row)}
            onopenforeignmodule={(root, row) => void openForeignModule(root, row)}
            onmodulecontext={(row, x, y, root) => void moduleContext(row, x, y, root)}
          />
        </Panel>
      </div>
      {/if}
      {#if shown.repositories && shown.worktrees}
      <Splitter
        direction="horizontal"
        value={1 - fractions.worktreesHeight}
        label="Resize worktrees panel"
        onchange={(d) =>
          layout.set(
            "worktreesHeight",
            worktreesHeightAfterDrag(fractions.worktreesHeight, d, leftColumnHeight, WORKTREES_MIN_PX),
          )}
        onreset={() => layout.resetOne("worktreesHeight")}
      />
      {/if}
      {#if shown.worktrees}
      <div
        class="pane worktrees-pane"
        class:grow={!shown.repositories}
        style:flex={shown.repositories ? `0 1 ${fractions.worktreesHeight * leftColumnHeight}px` : undefined}
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
              <button type="button" class="panel-act" title="Add Worktree…" onclick={() => worktreeActions?.openAdd()}
                >Add…</button
              >
              <button
                type="button"
                class="panel-act"
                disabled={!hasStale(worktrees.entries)}
                title={hasStale(worktrees.entries)
                  ? "Forget every worktree whose folder is gone"
                  : "No worktree is missing"}
                onclick={() => void worktreeActions?.pruneAll()}>Prune All</button
              >
            {/if}
          {/snippet}
          <WorktreesPanel
            onopen={(entry) => void openWorktreeRow(entry)}
            oncontext={(entry, x, y) => void worktreeActions?.context(entry, x, y)}
            onprune={(entry) => void worktreeActions?.prune(entry)}
            onrepair={(entry) => void worktreeActions?.repair(entry)}
            onadd={() => worktreeActions?.openAdd()}
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
            onactivate={activateRef}
            oncontext={(node, x, y) => void refContext(node, x, y)}
            ongroupcontext={(nodes, x, y) => void selectionActions?.refsContext(nodes, x, y)}
            onhover={(node) => { refActions?.prefetchNode(node); }}
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
            surface="editor"
            active={focused === "graph"}
            view={panelState}
            count={graph.total}
            busy={graph.loading}
            stale={stale.has("graph")}
          >
            {#snippet actions()}
              {#if repo}
                <GraphHeader
                  onchange={filterGraph}
                  matches={graph.total}
                  query={graph.query}
                  onpreferences={() => openSettings("graph")}
                />
              {/if}
            {/snippet}
            {#if describeSkipped(graph.skipped)}
              <p class="graph-skipped" role="status">{describeSkipped(graph.skipped)}</p>
            {/if}
            <GraphPanel
              {progress}
              check={checkCommand}
              oncheck={(command) => (checkCommand = command)}
              onruncheck={() => void runPauseCheck()}
              verdict={checkVerdict}
              {checking}
              ondrop={onCommitDrop}
              oncontext={(oid, x, y) => void commitContext(oid, x, y)}
              ongroupcontext={(oids, x, y) => void selectionActions?.commitsContext(oids, x, y)}
              onhover={(oid) => refActions?.prefetch(oid)}
              {banner}
              busy={repository.busy}
              onbanneraction={runBannerAction}
              onworktreecontext={(x, y) => void refActions?.worktreeContext(x, y)}
              onrefcontext={(label, oid, x, y) => void refActions?.labelContext(label, oid, x, y)}
              onactivate={(oid) => refActions?.checkOutCommit(oid)}
              onrefactivate={(label, oid) => refActions?.checkOutLabel(label, oid)}
              onclearfilter={() => filterGraph(parseQuery(""))}
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
            count={filesCount}
            stale={stale.has("files")}
          >
            <FilesPanel
              view={panelView(repository.phase)}
              activePanel={focused === "files"}
              {onWorkingTree}
              onviewchange={(next) => {
                const again = readsAgain(filesView.current, next);
                filesView.set(next);
                if (repo && again) void worktree.load(repo.repo);
              }}
              onopenworktree={openWorktreeDiff}
              onopenstaged={openStagedDiff}
              onopencommit={openDiff}
              onopenstash={openStashDiff}
              onopencompare={openCompareDiff}
              onopenwindow={openInWindow}
              onmask={(mask) => (fileMask = mask)}
              onshownstaged={(paths) => (shownStaged = paths)}
              onmarked={(paths, bySection) => {
                markedFiles = paths;
                markedBySection = bySection;
              }}
              oncount={(count) => (filesCount = count)}
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
        <Panel title="Diff" surface="editor" active={focused === "diff"} view={panelState} stale={stale.has("diff")}>
          <DiffPanel
            active={focused === "diff"}
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
              if (id && conflicts.path) openSolverAt(conflicts.path);
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
      commands={withShortcuts(palette, menuKeys, ON_MAC)}
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
      published={rebasePublished}
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
    {mutate}
    {reloadGraph}
    {checkOut}
    {openSplit}
    {openRebase}
    openAddWorktree={(origin) => worktreeActions?.openAdd(origin)}
    rollbackTree={() => rollbackFiles([])}
  />
  <BisectActions bind:this={bisectActions} {afterRefChange} />

  <SelectionActions bind:this={selectionActions} input={refTreeInput} {afterRefChange} {reloadGraph} />

  <RefGroupActions
    bind:this={refGroupActions}
    input={refTreeInput}
    {afterRefChange}
    {afterFetch}
    {reloadGraph}
    addTag={() => void refActions?.addTag(null)}
  />

  {#if split}
    <SplitOffDialog
      oid={split.oid}
      changed={split.changed}
      published={split.published}
      busy={splitBusy}
      onsplit={(paths, message, first) => void runSplit(paths, message, first)}
      onclose={() => (split = null)}
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
            validate: presetNameProblem,
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
      dirty={hooks.dirty}
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

  <SuccessToast />
  <Notifications
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
  <GitMissingDialog store={gitMissing} />
  <NetworkActions
    bind:this={networkActions}
    {tracked}
    afterMutation={() => afterMutation()}
    {afterFetch}
    {afterRefChange}
    pushNeedsDialog={() => refActions?.pushNeedsDialog() ?? false}
    {openedModule}
    {announceHooks}
    hooksDone={() => (runningHooks = undefined)}
  />
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

  <WorktreeActions
    bind:this={worktreeActions}
    openRow={openWorktreeRow}
    {copyText}
    graphFocused={focused === "graph"}
  />

  {#if welcome.open}
    <WelcomeDialog
      dialog={welcome}
      rows={welcomeRows}
      showAtStart={settings.current.startupShowWelcome}
      onshowchange={(show) => void settings.set("startupShowWelcome", show)}
      onrun={(action) => void runWelcome(action)}
      onforget={forgetWelcomePath}
      oncontext={(path, x, y) => void welcomeContext(path, x, y)}
      onclose={() => welcome.close()}
    />
  {/if}

  {#if cloneWizard.open}
    <CloneDialog
      wizard={cloneWizard}
      onbrowse={pickFolder}
      onfinish={(request) => void startClone(request)}
      onclose={() => cloneWizard.close()}
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
      bulk: networkActions?.bulkProgress(),
      network: network.running ?? undefined,
      networkProgress: network.progress ?? undefined,
      hooks: runningHooks,
      opening: repository.busy,
      failed: notices.errorCount + errorWindow.errorCount > 0,
    })}
    problems={output.problems}
    onproblems={() => output.toggle()}
    gitMissing={gitMissing.missing}
    ongitmissing={() => gitMissing.reopen(null)}
    oncancel={networkOps.length > 0 ? cancelNetworkOperation : undefined}
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
    background: var(--surface-base);
    opacity: 0.85;
    border: 2px dashed var(--status-ref);
    color: var(--text-primary);
    font-size: var(--fs-ui);
    pointer-events: none;
  }

  .workspace {
    position: relative;
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
