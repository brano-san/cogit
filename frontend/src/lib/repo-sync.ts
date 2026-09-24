import type { RepoOverview } from "$lib/ipc";
import type { RepoPulse } from "$lib/ipc/bindings";
import { DIRTY_REPOSITORY, trackTooltip } from "$lib/repo-labels";

/** The marks of one row of the Repositories list (R-353). */
export interface RowSync {
  /** `null` until known: the slot stays empty rather than claiming "clean". */
  dirty: boolean | null;
  ahead: number;
  behind: number;
  /** The last background fetch failed, so whether there is anything to pull is unknown. */
  unknown: boolean;
  missing: boolean;
}

export interface RowSyncInput {
  overview: RepoOverview | null;
  /** The panels show it: its row reads the full status they keep fresh. */
  owned: boolean;
  pulse: RepoPulse | undefined;
  fetchFailed: boolean;
}

/** The repository on screen from its full status; any other from its latest pulse, which
    the watcher and the queue keep fresher than the list's last read. */
export function rowSync({ overview, owned, pulse, fetchFailed }: RowSyncInput): RowSync {
  const source = owned && overview ? overview : (pulse ?? overview);
  if (!source) return { dirty: null, ahead: 0, behind: 0, unknown: fetchFailed, missing: false };
  if (source.missing) return { dirty: null, ahead: 0, behind: 0, unknown: false, missing: true };
  return {
    dirty: source.dirty,
    ahead: source.ahead,
    behind: source.behind,
    unknown: fetchFailed,
    missing: false,
  };
}

export const UNKNOWN_PULL =
  "The last background fetch failed, so whether there is anything to pull is unknown. The log says why.";

export function syncTooltip(sync: RowSync): string {
  const parts: string[] = [];
  if (sync.dirty) parts.push(DIRTY_REPOSITORY);
  const track = trackTooltip(sync.ahead, sync.behind);
  if (track) parts.push(track);
  if (sync.unknown) parts.push(UNKNOWN_PULL);
  return parts.join("\n");
}
