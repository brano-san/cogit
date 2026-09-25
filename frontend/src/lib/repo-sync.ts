import type { RepoOverview, RepoSummary } from "$lib/ipc";
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
  /** The server has commits HEAD lacks that are not fetched yet (R-354). */
  remoteAhead: boolean;
  missing: boolean;
}

export interface RowSyncInput {
  overview: RepoOverview | null;
  /** The panels show it: its row reads the full status they keep fresh. */
  owned: boolean;
  pulse: RepoPulse | undefined;
  fetchFailed: boolean;
  remoteAhead?: boolean;
}

/** The repository on screen from its full status; any other from its latest pulse, which
    the watcher and the queue keep fresher than the list's last read. */
export function rowSync(input: RowSyncInput): RowSync {
  const { overview, owned, pulse, fetchFailed } = input;
  const remoteAhead = input.remoteAhead === true && !fetchFailed;
  const source = owned && overview ? overview : (pulse ?? overview);
  const none = { dirty: null, ahead: 0, behind: 0, unknown: false, remoteAhead, missing: false };
  if (!source) return { ...none, unknown: fetchFailed };
  if (source.missing) return { ...none, remoteAhead: false, missing: true };
  return {
    dirty: source.dirty,
    ahead: source.ahead,
    behind: source.behind,
    unknown: fetchFailed,
    remoteAhead,
    missing: false,
  };
}

/** The list is read again on open, fetch and the like; what the panels show is read after
    every write and every change on disk, so the row on screen takes its marks from that. */
export function freshOverview(overview: RepoOverview, current: RepoSummary | null): RepoOverview {
  if (!current || current.repo !== overview.repo) return overview;
  const head = current.head.kind === "branch" ? current.head.name : null;
  const tracked = head === null ? undefined : current.branches.find((b) => b.kind === "local" && b.name === head);
  const { staged, unstaged, untracked, conflicted } = current.status;
  return {
    ...overview,
    branch: head,
    ahead: tracked?.ahead ?? 0,
    behind: tracked?.behind ?? 0,
    dirty: staged + unstaged + untracked + conflicted > 0,
    missing: false,
    state: current.state,
  };
}

/** The pull arrow: behind the tracking ref, or a server that moved on since the last fetch. */
export function canPull(sync: RowSync): boolean {
  return sync.behind > 0 || sync.remoteAhead;
}

export const UNKNOWN_PULL =
  "The remote could not be asked, so whether there is anything to pull is unknown. The log says why.";

export const REMOTE_AHEAD = "The remote has commits not fetched yet. Pull to get them.";

export function syncTooltip(sync: RowSync): string {
  const parts: string[] = [];
  if (sync.dirty) parts.push(DIRTY_REPOSITORY);
  const track = trackTooltip(sync.ahead, sync.behind);
  if (track) parts.push(track);
  if (sync.remoteAhead && sync.behind === 0) parts.push(REMOTE_AHEAD);
  if (sync.unknown) parts.push(UNKNOWN_PULL);
  return parts.join("\n");
}
