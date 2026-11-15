import type { Branch, Head } from "$lib/ipc";
import { splitUpstream } from "./push-to";

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
    const tracked = splitUpstream(upstream, remotes)?.remote;
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

/** Every remote but `current`, by name: what the fetch menu lists after it and Pull all fetches. */
function otherRemotes(remotes: readonly string[], current: string | null): string[] {
  return remotes.filter((remote) => remote !== current).sort((a, b) => a.localeCompare(b));
}

export function remotesInOrder(remotes: readonly string[], current: string | null): string[] {
  const others = otherRemotes(remotes, current);
  return current !== null && remotes.includes(current) ? [current, ...others] : others;
}

export function pullSteps(
  scope: PullScope,
  remotes: readonly string[],
  current: string,
): { fetch: string[]; pull: string } {
  return {
    fetch: scope === "all" ? otherRemotes(remotes, current) : [],
    pull: current,
  };
}

export type RemoteStep =
  | { kind: "fetch"; remote: string }
  | { kind: "pull"; remote: string }
  | { kind: "deleteMerged" }
  | { kind: "push"; remote: string };

export interface RemoteFacts {
  remotes: readonly string[];
  pullRemote: string | null;
  pushRemote: string | null;
  scope: PullScope;
  deleteMerged: boolean;
  /** The checked-out branch, as the last fetch left it. */
  branch?: { name: string; upstream: string | null; ahead: number; behind: number } | null;
  /** HEAD is detached: Pull only fetches (F-710). */
  detached?: boolean;
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
    // No branch to merge into: fetch only, as many remotes as Pull would, and never touch
    // HEAD or delete a branch (F-710).
    if (facts.detached) {
      const { fetch, pull } = pullSteps(facts.scope, facts.remotes, remote);
      plan.push(...[pull, ...fetch].map((name) => ({ kind: "fetch" as const, remote: name })));
      continue;
    }
    // Nothing to merge: the backend fetches every remote in this one step (R-552).
    if (facts.branch && facts.branch.upstream === null) {
      plan.push({ kind: "pull", remote });
      continue;
    }
    const { fetch, pull } = pullSteps(facts.scope, facts.remotes, remote);
    plan.push(...fetch.map((name) => ({ kind: "fetch" as const, remote: name })));
    plan.push({ kind: "pull", remote: pull });
    if (facts.deleteMerged) plan.push({ kind: "deleteMerged" });
  }
  return plan;
}
