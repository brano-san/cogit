import { reasonFor, type Context } from "./availability";
import type { PaletteCommand } from "./palette";
import { PANELS, type PanelId } from "./perspectives";
import { abortAction, type Banner } from "./repo-state";
import { reasonOf, type ToolbarFacts } from "./toolbar";

/** The palette's own commands, in the order it lists them; the native menu runs the same
    entries by id, and the toolbar's buttons of the same id share their action. Bisect and
    Remote ▸ … come from `bisectCommands` and `remoteCommands`. */

export const PANEL_TITLES: Record<PanelId, string> = {
  repositories: "Repositories",
  refs: "References",
  graph: "Graph",
  files: "Files",
  commit: "Commit Message",
  diff: "Diff",
  worktrees: "Worktrees",
};

export type FlowKind = "feature" | "release" | "hotfix";
const FLOW_KINDS: readonly FlowKind[] = ["feature", "release", "hotfix"];

export type AppCommandId =
  | "open"
  | "clone"
  | "welcome"
  | "fetch"
  | "pull"
  | "pull-defaults"
  | "push"
  | "push-defaults"
  | "stash"
  | "stash-selection"
  | "tag"
  | "stage"
  | "unstage"
  | "commit"
  | "commit-amend"
  | "commit-window"
  | "commit-message"
  | "undo"
  | "output"
  | "copy-path"
  | "copy-branch"
  | "copy-sha"
  | "rebase-i"
  | "flow-init"
  | "flow-finish"
  | "split-off"
  | "rollback"
  | "close"
  | "refresh"
  | "branch"
  | "reset-layout"
  | "overlap"
  | "avatars"
  | "hooks"
  | "perspective-main"
  | "perspective-review"
  | "maximize-panel"
  | "worktree-add"
  | "worktree-remove"
  | "worktree-prune"
  | "resolve-conflicts"
  | "undo-rewrite"
  | "range-diff"
  | "maintenance-gc"
  | "palette"
  | "check-updates"
  | "edit-config-repository"
  | "edit-config-user"
  | "exit"
  | "about"
  | "settings"
  | "find"
  | "pr"
  | "scan"
  | "journal"
  | "fetch-all"
  | "reveal-log"
  | "copy-pr"
  | "blame"
  | "abort";

/** One action per id; a toolbar button busies itself while the promise it returns runs. */
export type AppCommandActions = Record<AppCommandId, () => unknown> & {
  flowStart: (kind: FlowKind) => void;
  togglePanel: (panel: PanelId) => void;
};

/** What the rules read; data only, so they can be tested without the app. */
export interface AppCommandFacts {
  context: Context;
  toolbar: ToolbarFacts;
  lastUndo: boolean;
  diffPath: string | null;
  /** HEAD is on a branch the app tracks. */
  branch: boolean;
  commitOid: string | null;
  flow: { initialised: boolean; current: boolean };
  /** Branches that already hold the selected commit. */
  protectedBy: readonly string[];
  /** The selected commit is on the checked-out branch; unknown counts as yes, the engine refuses. */
  onHead: boolean;
  worktrees: { removable: boolean; stale: boolean };
  conflicts: number;
  prReason: string | undefined;
  logPath: boolean;
  banner: Banner | null;
  openCount: number;
}

const SELECT_COMMIT = "Select a commit first";
const NO_FILE_IN_DIFF = "No file is open in the Diff panel";

export function appCommands(facts: AppCommandFacts, run: AppCommandActions): PaletteCommand[] {
  const noRepo = reasonFor({ repository: true }, facts.context);
  const noRemote = reasonFor({ remote: true }, facts.context);
  const toolbar = (id: string) => reasonOf(id, facts.toolbar);
  const noCommit = facts.commitOid ? undefined : SELECT_COMMIT;
  const notOnHead = facts.onHead ? undefined : "Not on the current branch";

  return [
    { id: "open", title: "Open Repository…", run: run.open },
    {
      id: "clone",
      title: "Clone Repository…",
      synonyms: ["git clone", "download a repository", "check out from a server"],
      run: run.clone,
    },
    {
      id: "welcome",
      title: "Welcome…",
      synonyms: ["start", "recent repositories", "reopen", "new repository", "git init"],
      run: run.welcome,
    },
    { id: "fetch", title: "Fetch", unavailable: noRepo ?? noRemote, run: run.fetch },
    { id: "pull", title: "Pull…", unavailable: toolbar("pull"), run: run.pull },
    { id: "pull-defaults", title: "Pull with Defaults", unavailable: toolbar("pull"), run: run["pull-defaults"] },
    { id: "push", title: "Push…", unavailable: toolbar("push"), run: run.push },
    { id: "push-defaults", title: "Push with Defaults", unavailable: toolbar("push"), run: run["push-defaults"] },
    { id: "stash", title: "Stash All", synonyms: ["shelve"], unavailable: toolbar("stash"), run: run.stash },
    {
      id: "stash-selection",
      title: "Stash Selection",
      synonyms: ["shelve some"],
      // The ticks may be a commit's files now; only the working tree's can be stashed.
      unavailable: noRepo ?? toolbar("stash-selection"),
      run: run["stash-selection"],
    },
    { id: "tag", title: "Create Tag", unavailable: toolbar("tag"), run: run.tag },
    { id: "stage", title: "Stage", unavailable: toolbar("stage"), run: run.stage },
    { id: "unstage", title: "Unstage", unavailable: toolbar("unstage"), run: run.unstage },
    // The box decides whether it can commit: with Amend ticked nothing has to be staged.
    { id: "commit", title: "Commit Staged", unavailable: noRepo, run: run.commit },
    { id: "commit-amend", title: "Commit with Amend", unavailable: noRepo, run: run["commit-amend"] },
    { id: "commit-window", title: "Commit…", unavailable: noRepo, run: run["commit-window"] },
    { id: "commit-message", title: "Go to the Commit Message", unavailable: noRepo, run: run["commit-message"] },
    {
      id: "undo",
      title: "Undo Last Operation",
      unavailable: facts.lastUndo ? undefined : "Nothing to undo",
      run: run.undo,
    },
    { id: "output", title: "Toggle Output Panel", run: run.output },
    {
      id: "copy-path",
      title: "Copy the File Path",
      unavailable: facts.diffPath ? undefined : NO_FILE_IN_DIFF,
      run: run["copy-path"],
    },
    {
      id: "copy-branch",
      title: "Copy the Branch Name",
      unavailable: facts.branch ? undefined : "HEAD is not on a branch",
      run: run["copy-branch"],
    },
    { id: "copy-sha", title: "Copy the Commit SHA", unavailable: noCommit, run: run["copy-sha"] },
    {
      id: "rebase-i",
      title: "Rebase Commits After This One…",
      synonyms: ["interactive rebase", "squash", "reorder"],
      unavailable: noCommit ?? notOnHead,
      run: run["rebase-i"],
    },
    {
      id: "flow-init",
      title: "Git-Flow: Set Up",
      synonyms: ["gitflow", "develop branch"],
      unavailable: noRepo ?? (facts.flow.initialised ? "Already set up" : undefined),
      run: run["flow-init"],
    },
    ...FLOW_KINDS.map((kind) => ({
      id: `flow-${kind}`,
      title: `Git-Flow: Start ${kind[0]?.toUpperCase()}${kind.slice(1)}…`,
      synonyms: ["gitflow", kind],
      unavailable: noRepo ?? (facts.flow.initialised ? undefined : "Set up Git-Flow first"),
      run: () => run.flowStart(kind),
    })),
    {
      id: "flow-finish",
      title: "Git-Flow: Finish This Branch…",
      synonyms: ["gitflow", "merge back"],
      unavailable: facts.flow.current ? undefined : "Not on a Git-Flow branch",
      run: run["flow-finish"],
    },
    {
      id: "split-off",
      title: "Split Off Files…",
      synonyms: ["split commit", "surgery"],
      unavailable:
        noCommit ??
        notOnHead ??
        (facts.protectedBy.length > 0 ? `Already on ${facts.protectedBy.join(", ")}` : undefined),
      run: run["split-off"],
    },
    {
      id: "rollback",
      title: "Roll Back Tree To This Commit",
      synonyms: ["restore", "revert files"],
      unavailable: noCommit,
      run: run.rollback,
    },
    { id: "close", title: "Close Repository", unavailable: noRepo, run: run.close },
    { id: "refresh", title: "Refresh", unavailable: noRepo, run: run.refresh },
    { id: "branch", title: "New Branch…", unavailable: noRepo, run: run.branch },
    { id: "reset-layout", title: "Reset Perspective", run: run["reset-layout"] },
    {
      id: "overlap",
      title: "Toggle Commit Overlap Column",
      synonyms: ["who else touched", "conflict risk"],
      run: run.overlap,
    },
    {
      id: "avatars",
      title: "Toggle Author Avatars",
      synonyms: ["gravatar", "pictures", "faces"],
      run: run.avatars,
    },
    {
      id: "hooks",
      title: "Manage Hooks…",
      synonyms: ["pre-commit", "git hooks"],
      unavailable: noRepo,
      run: run.hooks,
    },
    { id: "perspective-main", title: "Perspective: Main", run: run["perspective-main"] },
    { id: "perspective-review", title: "Perspective: Review", run: run["perspective-review"] },
    {
      id: "maximize-panel",
      title: "Maximize Panel",
      synonyms: ["zoom", "full screen panel"],
      run: run["maximize-panel"],
    },
    {
      id: "worktree-add",
      title: "Add Worktree…",
      synonyms: ["worktree", "second checkout"],
      unavailable: noRepo,
      run: run["worktree-add"],
    },
    {
      id: "worktree-remove",
      title: "Remove Worktree…",
      unavailable:
        noRepo ??
        (facts.worktrees.removable ? undefined : "Select a worktree other than the main one in the Worktrees panel"),
      run: run["worktree-remove"],
    },
    {
      id: "worktree-prune",
      title: "Prune Obsolete Worktrees…",
      synonyms: ["worktree prune", "missing worktree"],
      unavailable: noRepo ?? (facts.worktrees.stale ? undefined : "Nothing to prune"),
      run: run["worktree-prune"],
    },
    {
      id: "resolve-conflicts",
      title: "Resolve Conflicts…",
      synonyms: ["conflict solver", "merge", "3-way", "resolve", "conflicted"],
      unavailable: noRepo ?? (facts.conflicts === 0 ? "No file is conflicted" : undefined),
      run: run["resolve-conflicts"],
    },
    {
      id: "undo-rewrite",
      title: "Undo Last Merge / Rebase / Reset…",
      synonyms: ["ORIG_HEAD", "undo merge", "undo rebase", "undo reset"],
      unavailable: noRepo,
      run: run["undo-rewrite"],
    },
    {
      id: "range-diff",
      title: "Compare Before and After Rewrite",
      synonyms: ["range-diff", "rebase", "ORIG_HEAD"],
      unavailable: noRepo,
      run: run["range-diff"],
    },
    {
      id: "maintenance-gc",
      title: "Run Maintenance (gc)…",
      synonyms: ["gc", "garbage collect", "housekeeping", "repack"],
      unavailable: noRepo,
      run: run["maintenance-gc"],
    },
    ...PANELS.map((panel) => ({
      id: `panel-${panel}`,
      title: `Toggle ${PANEL_TITLES[panel]} Panel`,
      run: () => run.togglePanel(panel),
    })),
    { id: "palette", title: "Find Command", run: run.palette },
    { id: "check-updates", title: "Check for Updates", run: run["check-updates"] },
    {
      id: "edit-config-repository",
      title: "Edit Git Config: Repository",
      synonyms: ["config", ".git/config", "settings", "remote"],
      unavailable: noRepo,
      run: run["edit-config-repository"],
    },
    {
      id: "edit-config-user",
      title: "Edit Git Config: User",
      synonyms: ["config", "gitconfig", "global", "user.name", "email"],
      run: run["edit-config-user"],
    },
    { id: "exit", title: "Exit", synonyms: ["quit", "close"], run: run.exit },
    { id: "about", title: "About Cogit", run: run.about },
    {
      id: "settings",
      title: "Preferences",
      synonyms: ["settings", "options", "customize toolbar", "toolbar buttons"],
      run: run.settings,
    },
    { id: "find", title: "Find Object", synonyms: ["goto", "jump"], unavailable: noRepo, run: run.find },
    {
      id: "pr",
      title: "Create Pull Request",
      synonyms: ["merge request", "pr", "mr"],
      unavailable: facts.prReason,
      run: run.pr,
    },
    {
      id: "scan",
      title: "Scan Folder for Repositories",
      synonyms: ["discover", "find repositories", "import"],
      run: run.scan,
    },
    {
      id: "journal",
      title: "Safety Journal",
      synonyms: ["undo history", "recover", "what did I just do"],
      unavailable: noRepo,
      run: run.journal,
    },
    {
      id: "fetch-all",
      title: "Fetch All",
      synonyms: ["update every repository"],
      unavailable: facts.openCount > 0 ? undefined : "No repository is open",
      run: run["fetch-all"],
    },
    {
      id: "reveal-log",
      title: "Reveal Log File",
      synonyms: ["profiling", "diagnostics", "performance"],
      unavailable: facts.logPath ? undefined : "The log path is not known yet",
      run: run["reveal-log"],
    },
    { id: "copy-pr", title: "Copy Pull Request Link", unavailable: facts.prReason, run: run["copy-pr"] },
    {
      id: "blame",
      title: "Blame This File",
      unavailable: facts.diffPath ? undefined : NO_FILE_IN_DIFF,
      run: run.blame,
    },
    {
      id: "abort",
      title: "Abort Operation In Progress",
      unavailable: abortAction(facts.banner) ? undefined : "Nothing is in progress",
      run: run.abort,
    },
  ];
}
