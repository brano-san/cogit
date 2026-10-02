import { describe, expect, it, vi } from "vitest";
import { appCommands, type AppCommandActions, type AppCommandFacts } from "./app-commands";
import { NOTHING } from "./availability";
import type { Banner } from "./repo-state";
import { NO_FACTS } from "./toolbar";

/** One spy per key, the same one on every read. */
function spies(): AppCommandActions & Record<string, ReturnType<typeof vi.fn>> {
  const made = new Map<string, ReturnType<typeof vi.fn>>();
  return new Proxy({} as AppCommandActions & Record<string, ReturnType<typeof vi.fn>>, {
    get: (_, key: string) => {
      if (!made.has(key)) made.set(key, vi.fn());
      return made.get(key);
    },
  });
}

const OPEN: AppCommandFacts = {
  context: { ...NOTHING, repository: true, remote: true, commit: true, branch: true },
  toolbar: {
    ...NO_FACTS,
    repository: true,
    remote: true,
    onWorkingTree: true,
    markedUnstaged: ["a.txt"],
    markedStaged: ["b.txt"],
    unstaged: ["a.txt"],
    staged: ["b.txt"],
    commit: "abc",
    head: "abc",
    branch: true,
    upstream: true,
  },
  lastUndo: true,
  diffPath: "a.txt",
  branch: true,
  commitOid: "abc",
  flow: { initialised: true, current: true },
  protectedBy: [],
  worktrees: { removable: true, stale: true },
  conflicts: 1,
  prReason: undefined,
  logPath: true,
  banner: null,
  openCount: 1,
};

const CLOSED: AppCommandFacts = {
  ...OPEN,
  context: NOTHING,
  toolbar: NO_FACTS,
  commitOid: null,
  openCount: 0,
};

const reason = (facts: AppCommandFacts, id: string) =>
  appCommands(facts, spies()).find((command) => command.id === id)?.unavailable;

describe("app commands", () => {
  it("has unique ids", () => {
    const ids = appCommands(OPEN, spies()).map((command) => command.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  // Copied from the palette literal App.svelte held before it moved here.
  it("keeps the palette's order and titles", () => {
    expect(appCommands(OPEN, spies()).map((command) => [command.id, command.title])).toEqual([
      ["open", "Open Repository…"],
      ["clone", "Clone Repository…"],
      ["welcome", "Welcome…"],
      ["fetch", "Fetch"],
      ["pull", "Pull…"],
      ["pull-defaults", "Pull with Defaults"],
      ["push", "Push…"],
      ["push-defaults", "Push with Defaults"],
      ["stash", "Stash All"],
      ["stash-selection", "Stash Selection"],
      ["tag", "Create Tag"],
      ["stage", "Stage"],
      ["unstage", "Unstage"],
      ["commit", "Commit Staged"],
      ["commit-amend", "Commit with Amend"],
      ["commit-window", "Commit…"],
      ["commit-message", "Go to the Commit Message"],
      ["undo", "Undo Last Operation"],
      ["output", "Toggle Output Panel"],
      ["copy-path", "Copy the File Path"],
      ["copy-branch", "Copy the Branch Name"],
      ["copy-sha", "Copy the Commit SHA"],
      ["rebase-i", "Rebase Commits After This One…"],
      ["flow-init", "Git-Flow: Set Up"],
      ["flow-feature", "Git-Flow: Start Feature…"],
      ["flow-release", "Git-Flow: Start Release…"],
      ["flow-hotfix", "Git-Flow: Start Hotfix…"],
      ["flow-finish", "Git-Flow: Finish This Branch…"],
      ["split-off", "Split Off Files…"],
      ["rollback", "Roll Back Tree To This Commit"],
      ["close", "Close Repository"],
      ["refresh", "Refresh"],
      ["branch", "New Branch…"],
      ["reset-layout", "Reset Perspective"],
      ["overlap", "Toggle Commit Overlap Column"],
      ["avatars", "Toggle Author Avatars"],
      ["hooks", "Manage Hooks…"],
      ["perspective-main", "Perspective: Main"],
      ["perspective-review", "Perspective: Review"],
      ["maximize-panel", "Maximize Panel"],
      ["worktree-add", "Add Worktree…"],
      ["worktree-remove", "Remove Worktree…"],
      ["worktree-prune", "Prune Obsolete Worktrees…"],
      ["resolve-conflicts", "Resolve Conflicts…"],
      ["undo-rewrite", "Undo Last Merge / Rebase / Reset…"],
      ["range-diff", "Compare Before and After Rewrite"],
      ["maintenance-gc", "Run Maintenance (gc)…"],
      ["panel-repositories", "Toggle Repositories Panel"],
      ["panel-refs", "Toggle References Panel"],
      ["panel-graph", "Toggle Graph Panel"],
      ["panel-files", "Toggle Files Panel"],
      ["panel-commit", "Toggle Commit Message Panel"],
      ["panel-diff", "Toggle Diff Panel"],
      ["panel-worktrees", "Toggle Worktrees Panel"],
      ["palette", "Find Command"],
      ["check-updates", "Check for Updates"],
      ["edit-config-repository", "Edit Git Config: Repository"],
      ["edit-config-user", "Edit Git Config: User"],
      ["exit", "Exit"],
      ["about", "About Cogit"],
      ["settings", "Preferences"],
      ["find", "Find Object"],
      ["pr", "Create Pull Request"],
      ["scan", "Scan Folder for Repositories"],
      ["journal", "Safety Journal"],
      ["fetch-all", "Fetch All"],
      ["reveal-log", "Reveal Log File"],
      ["copy-pr", "Copy Pull Request Link"],
      ["blame", "Blame This File"],
      ["abort", "Abort Operation In Progress"],
    ]);
  });

  it("runs the action of its own id", () => {
    const actions = spies();
    for (const command of appCommands(OPEN, actions)) {
      command.run();
      if (command.id.startsWith("panel-")) {
        expect(actions.togglePanel).toHaveBeenLastCalledWith(command.id.slice("panel-".length));
      } else if (["flow-feature", "flow-release", "flow-hotfix"].includes(command.id)) {
        expect(actions.flowStart).toHaveBeenLastCalledWith(command.id.slice("flow-".length));
      } else {
        expect(command.run).toBe(actions[command.id]);
      }
    }
  });

  it("says no repository is open for what needs one", () => {
    for (const id of ["fetch", "stash-selection", "commit", "close", "refresh", "find", "journal", "fetch-all"]) {
      expect(reason(CLOSED, id), id).toBe("No repository is open");
    }
    for (const id of ["open", "clone", "welcome", "output", "palette", "settings", "exit"]) {
      expect(reason(CLOSED, id), id).toBeUndefined();
    }
  });

  it("offers everything with a repository, a commit and a remote", () => {
    const blocked = appCommands(OPEN, spies()).filter((command) => command.unavailable);
    // Set Up wants Git-Flow not set up yet; Abort wants an operation in progress.
    expect(blocked.map((command) => command.id)).toEqual(["flow-init", "abort"]);
  });

  it("splits off only a commit no branch holds yet", () => {
    expect(reason({ ...OPEN, commitOid: null }, "split-off")).toBe("Select a commit first");
    expect(reason({ ...OPEN, protectedBy: ["main", "dev"] }, "split-off")).toBe("Already on main, dev");
    expect(reason(OPEN, "split-off")).toBeUndefined();
  });

  it("starts Git-Flow branches only once it is set up", () => {
    const fresh = { ...OPEN, flow: { initialised: false, current: false } };
    expect(reason(fresh, "flow-init")).toBeUndefined();
    expect(reason(fresh, "flow-feature")).toBe("Set up Git-Flow first");
    expect(reason(fresh, "flow-finish")).toBe("Not on a Git-Flow branch");
    expect(reason(OPEN, "flow-init")).toBe("Already set up");
    expect(reason(OPEN, "flow-hotfix")).toBeUndefined();
  });

  it("aborts only what the banner can abort", () => {
    const banner = (actions: Banner["actions"]): Banner => ({ title: "", detail: "", severity: "info", actions });
    expect(reason(OPEN, "abort")).toBe("Nothing is in progress");
    expect(reason({ ...OPEN, banner: banner(["continue"]) }, "abort")).toBe("Nothing is in progress");
    expect(reason({ ...OPEN, banner: banner(["abort"]) }, "abort")).toBeUndefined();
    expect(reason({ ...OPEN, banner: banner(["resetBisect"]) }, "abort")).toBeUndefined();
  });
});
