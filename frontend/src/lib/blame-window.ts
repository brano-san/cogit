import { shortOid } from "$lib/format";
import { openBlameWindow, type CommitRow, type RepoId } from "$lib/ipc";

export const BLAME_ROW_HEIGHT = 18;

export interface BlameRequest {
  repo: RepoId;
  path: string;
  /** The full hash the window was opened at; `open_blame_window` resolved it. */
  rev: string;
}

/** The one entry point for blame: every menu item, button and shortcut comes here. `rev`
    may be any revision — `HEAD` for the working tree. */
export async function openBlame(repo: RepoId, path: string, rev: string): Promise<void> {
  await openBlameWindow(repo, path, rev);
}

/** Built by `blame_window::url` in Rust; read here once, when the window starts. */
export function parseBlame(search: string): BlameRequest | null {
  const params = new URLSearchParams(search);
  const repo = Number(params.get("repo"));
  const path = params.get("path");
  const rev = params.get("rev");
  if (!Number.isInteger(repo) || repo <= 0 || !path || !rev) return null;
  return { repo, path, rev };
}

type Option = [string, string];

function revisionText(row: CommitRow, date: string): string {
  return `${date} · ${shortOid(row.oid)} · ${row.authorName} · ${row.summary}`;
}

/** `View Commit`: every version of the file, and the one the window was opened at even
    when that commit left the file alone. */
export function viewOptions(
  revisions: readonly CommitRow[],
  opened: string,
  date: (row: CommitRow) => string,
): Option[] {
  const options = revisions.map((row): Option => [row.oid, revisionText(row, date(row))]);
  if (revisions.some((row) => row.oid === opened)) return options;
  return [[opened, `${shortOid(opened)} · the version opened`], ...options];
}

/** `Highlight: Changes Since`; the empty value highlights nothing. */
export function sinceOptions(revisions: readonly CommitRow[], date: (row: CommitRow) => string): Option[] {
  return [["", "Nothing"], ...revisions.map((row): Option => [row.oid, revisionText(row, date(row))])];
}

/** The commits above `since` in the newest-first list of versions: what changed the file
    after it. Exact on a straight history; on a merged one the list is by date (R-202). */
export function changedSince(
  revisions: readonly { oid: string }[],
  since: string | null,
): Set<string> {
  const at = since === null ? -1 : revisions.findIndex((row) => row.oid === since);
  return new Set(at <= 0 ? [] : revisions.slice(0, at).map((row) => row.oid));
}

/** The commit to show for a line; `null` past the end or for a line no commit wrote yet,
    which git blames on the all-zero id. */
export function commitOfLine(lines: readonly { oid: string }[], at: number): string | null {
  const oid = lines[at]?.oid;
  return oid === undefined || /^0+$/.test(oid) ? null : oid;
}

/** Only the first line of a run from one commit carries the annotation, as `git blame`. */
export function startsBlock(lines: readonly { oid: string }[], at: number): boolean {
  return at === 0 || lines[at - 1]?.oid !== lines[at]?.oid;
}

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const YEAR = 365 * DAY;

/** The history column's age, SmartGit's short way: `45m`, `5h`, `140d`, `3y`. */
export function age(timestamp: number, now: number): string {
  const elapsed = Math.max(now - timestamp, 0);
  if (elapsed >= YEAR) return `${Math.floor(elapsed / YEAR)}y`;
  if (elapsed >= DAY) return `${Math.floor(elapsed / DAY)}d`;
  if (elapsed >= HOUR) return `${Math.floor(elapsed / HOUR)}h`;
  if (elapsed >= MINUTE) return `${Math.floor(elapsed / MINUTE)}m`;
  return "now";
}

/** Where the current line goes on a key; `null` for keys the list does not move on. */
export function cursorAfter(key: string, current: number, count: number, page: number): number | null {
  if (count === 0) return null;
  const last = count - 1;
  const moves: Record<string, number> = {
    ArrowDown: current + 1,
    ArrowUp: current - 1,
    PageDown: current + page,
    PageUp: current - page,
    Home: 0,
    End: last,
  };
  const next = moves[key];
  return next === undefined ? null : Math.min(Math.max(next, 0), last);
}

/** A 1-based line number from another version, as an index that exists in this one. */
export function clampLine(line: number, count: number): number {
  if (count === 0) return 0;
  return Math.min(Math.max(line - 1, 0), count - 1);
}
