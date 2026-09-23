/** What the toolbar remembers between runs, under the `toolbar` key of the settings file. */

export type PullScope = "current" | "all";

export interface ToolbarPrefs {
  /** Whether the main Pull button fetches every remote first (#26). */
  pullScope: PullScope;
  /** Deletes local branches merged into HEAD whose upstream the remote deleted (#26). */
  deleteMergedAfterPull: boolean;
}

export const DEFAULT_PREFS: ToolbarPrefs = {
  pullScope: "current",
  deleteMergedAfterPull: false,
};

/** Every stored value is checked on its own: one bad key keeps the others. */
export function mergePrefs(stored: unknown): ToolbarPrefs {
  const merged = { ...DEFAULT_PREFS };
  if (typeof stored !== "object" || stored === null) return merged;
  const value = stored as Record<string, unknown>;
  if (value.pullScope === "current" || value.pullScope === "all") merged.pullScope = value.pullScope;
  if (typeof value.deleteMergedAfterPull === "boolean") {
    merged.deleteMergedAfterPull = value.deleteMergedAfterPull;
  }
  return merged;
}

/**
 * The remote Pull uses: the one the current branch tracks, else `origin`, else the first.
 * Remote names may contain `/`, so the longest name that prefixes the upstream wins.
 */
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

/** The current remote first, the others in alphabetical order. */
export function remotesInOrder(remotes: readonly string[], current: string | null): string[] {
  const others = remotes.filter((remote) => remote !== current).sort((a, b) => a.localeCompare(b));
  return current !== null && remotes.includes(current) ? [current, ...others] : others;
}

/** What one click on Pull runs: the fetches first, then the pull from the current remote. */
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
