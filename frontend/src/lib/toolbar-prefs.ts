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
