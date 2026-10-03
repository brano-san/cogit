import type { ChangeKind, RepoId } from "$lib/ipc";
import { planFor } from "$lib/disk-change";
import type { PanelId } from "$lib/perspectives";

export interface DiskPassContext {
  repo: () => RepoId | null;
  /** `repository.epoch`: a change of it means the panels show another repository now. */
  epoch: () => number;
  /** Whether a commit is selected, in which case the Files panel shows that commit. */
  commitSelected: () => boolean;
  /** The refs and the state, read without going through `opening` (R-316). */
  refreshRefs: () => Promise<void>;
  resetProtection: () => void;
  hooksOpen: () => boolean;
  refreshHooks: (repo: RepoId) => void;
  loadWorktree: (repo: RepoId) => Promise<void>;
  refreshWorktrees: (repo: RepoId) => void;
  /** Reads the status along with everything hanging off a mutation. */
  afterMutation: () => Promise<void>;
  refreshDiff: () => Promise<void>;
  reselectCommit: (repo: RepoId) => void;
  loadGraph: (repo: RepoId) => Promise<void>;
  freshened: (panels: PanelId[]) => void;
}

/** One answer to a burst of watcher events. The refs are read without the repository going
    through `opening` (it would show «Opening repository…» and lock the banner buttons for
    somebody else's `git commit`), and the graph starts as soon as they are in, instead of
    waiting for the file list and the status (doc/12-risks.md, R-316). */
export async function runDiskPass(context: DiskPassContext, kinds: ReadonlySet<ChangeKind>): Promise<void> {
  const id = context.repo();
  const epoch = context.epoch();
  const left = () => context.epoch() !== epoch;
  const plan = planFor(kinds);
  if (!id) return;

  // A hook edited outside Cogit is only interesting while the panel is open.
  if (plan.hooks && context.hooksOpen()) context.refreshHooks(id);
  if (!plan.cascade) return;

  let graphLoad: Promise<void> | undefined;
  if (plan.refs) {
    context.resetProtection();
    await context.refreshRefs();
    if (left()) return;
    graphLoad = context.loadGraph(id);
  }
  context.freshened(["repositories", "refs"]);

  if (plan.worktree && !context.commitSelected()) {
    await context.loadWorktree(id);
    if (left()) return;
    context.freshened(["files", "commit"]);
  }
  context.refreshWorktrees(id);

  // `afterMutation` reads the status; the file list above read its own copy.
  await context.afterMutation();
  if (left()) return;
  context.freshened(["files", "commit"]);
  if (plan.worktree || plan.refs) await context.refreshDiff();
  if (left()) return;
  context.freshened(["diff"]);

  if (plan.authors) {
    context.reselectCommit(id);
    graphLoad ??= context.loadGraph(id);
  }
  await graphLoad;
  context.freshened(["graph", "refs"]);
}
