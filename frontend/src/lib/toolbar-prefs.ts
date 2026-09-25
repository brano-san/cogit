import type { Branch, Head } from "$lib/ipc";

export type PullScope = "current" | "all";

export type SyncOrder = "pullThenPush" | "pushThenPull";

export interface ToolbarPrefs {
  pullScope: PullScope;
  deleteMergedAfterPull: boolean;
  syncOrder: SyncOrder;
}

export const DEFAULT_PREFS: ToolbarPrefs = {
  pullScope: "current",
  deleteMergedAfterPull: false,
  syncOrder: "pullThenPush",
};

export function mergePrefs(stored: unknown): ToolbarPrefs {
  const merged = { ...DEFAULT_PREFS };
  if (typeof stored !== "object" || stored === null) return merged;
  const value = stored as Record<string, unknown>;
  if (value.pullScope === "current" || value.pullScope === "all") merged.pullScope = value.pullScope;
  if (typeof value.deleteMergedAfterPull === "boolean") {
    merged.deleteMergedAfterPull = value.deleteMergedAfterPull;
  }
  if (value.syncOrder === "pullThenPush" || value.syncOrder === "pushThenPull") {
    merged.syncOrder = value.syncOrder;
  }
  return merged;
}

/** The two halves of Sync in the order chosen; the second runs only if the first worked. */
export function syncSteps(order: SyncOrder): ("pull" | "push")[] {
  return order === "pushThenPull" ? ["push", "pull"] : ["pull", "push"];
}

/** The tracked remote, else `origin`, else the first; a name may hold `/`: longest wins. */
export function currentRemote(
  upstream: string | null | undefined,
  remotes: readonly string[],
): string | null {
  if (upstream) {
    const tracked = remotes
      .filter((remote) => upstream.startsWith(`${remote}/`))
      .sort((a, b) => b.length - a.length)[0];
    if (tracked) return tracked;
  }
  return remotes.includes("origin") ? "origin" : (remotes[0] ?? null);
}

/** `currentRemote` of a repository the panels do not show, from its refs alone. */
export function headRemote(
  refs: { head: Head; branches: readonly Branch[] },
  remotes: readonly string[],
): string | null {
  const head = refs.head.kind === "branch" ? refs.head.name : null;
  const branch = refs.branches.find((each) => each.kind === "local" && each.name === head);
  return currentRemote(branch?.upstream, remotes);
}

export function remotesInOrder(remotes: readonly string[], current: string | null): string[] {
  const others = remotes.filter((remote) => remote !== current).sort((a, b) => a.localeCompare(b));
  return current !== null && remotes.includes(current) ? [current, ...others] : others;
}

export function pullSteps(
  scope: PullScope,
  remotes: readonly string[],
  current: string,
): { fetch: string[]; pull: string } {
  return {
    fetch: scope === "all" ? remotesInOrder(remotes, current).filter((r) => r !== current) : [],
    pull: current,
  };
}

export type RemoteStep =
  | { kind: "fetch"; remote: string }
  | { kind: "pull"; remote: string; ffOnly: boolean }
  | { kind: "deleteMerged" }
  | { kind: "push"; remote: string };

export interface RemoteFacts {
  remotes: readonly string[];
  pullRemote: string | null;
  pushRemote: string | null;
  scope: PullScope;
  ffOnly: boolean;
  deleteMerged: boolean;
  /** The checked-out branch, as the last fetch left it. */
  branch?: { name: string; upstream: string | null; ahead: number; behind: number } | null;
}

/** A pull that is sure to fail says so before it starts: `--ff-only` cannot join two
    branches that have both moved (F-320, R-456). */
function pullProblem(facts: RemoteFacts): string | null {
  const branch = facts.branch;
  if (!facts.ffOnly || !branch?.upstream || branch.ahead === 0 || branch.behind === 0) return null;
  return (
    `${branch.name} and ${branch.upstream} have diverged (${branch.ahead} ahead, ${branch.behind} behind), ` +
    "and Pull only fast-forwards (Preferences ▸ Pull). Merge or rebase the branch first, " +
    "or let Pull merge it."
  );
}

/** Every step of a pull, a push or a Sync, decided before the first one runs: each takes
    seconds, and by the time one ends the panels may show another repository. */
export function remotePlan(steps: readonly ("pull" | "push")[], facts: RemoteFacts): RemoteStep[] {
  const plan: RemoteStep[] = [];
  for (const step of steps) {
    const remote = step === "pull" ? facts.pullRemote : facts.pushRemote;
    if (!remote) throw new Error("This repository has no remote.");
    if (step === "push") {
      plan.push({ kind: "push", remote });
      continue;
    }
    const problem = pullProblem(facts);
    if (problem) throw new Error(problem);
    const { fetch, pull } = pullSteps(facts.scope, facts.remotes, remote);
    plan.push(...fetch.map((name) => ({ kind: "fetch" as const, remote: name })));
    plan.push({ kind: "pull", remote: pull, ffOnly: facts.ffOnly });
    if (facts.deleteMerged) plan.push({ kind: "deleteMerged" });
  }
  return plan;
}
