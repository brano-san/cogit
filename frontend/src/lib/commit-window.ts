import type { FileEntry } from "$lib/ipc";
import type { StateSide } from "$lib/file-state";
import { commitBlocker } from "$lib/commit-draft";

/** Which files the window lists: what is staged, or every local change (staged or not). */
export type CommitMode = "staged" | "local";

export interface CommitWindowRequest {
  repo: number;
  /** Keys the message draft the inline panel shares (`cogit:draft:<root>`). */
  root: string;
  /** The files it was opened for (Files ▸ Commit…); none: everything listed. */
  paths: string[];
}

/** Built by `commit_window::url` in Rust; read once, when the window starts. */
export function parseCommitWindow(search: string): CommitWindowRequest | null {
  const params = new URLSearchParams(search);
  const repo = Number(params.get("repo"));
  const root = params.get("root");
  if (!Number.isInteger(repo) || repo <= 0 || root === null) return null;
  return { repo, root, paths: params.getAll("path") };
}

/** What the window opens on. For chosen files: Local Changes if one has unstaged changes (the
    commit takes them, as staging the file would), else Staged; exactly those ticked. Without:
    Staged with all ticked, or Local Changes when nothing is staged. */
export function opening(
  paths: readonly string[],
  staged: readonly FileEntry[],
  unstaged: readonly FileEntry[],
): { mode: CommitMode; ticked: string[] } {
  const listed = (mode: CommitMode) => listedFiles(mode, staged, unstaged).map((file) => file.path);
  if (paths.length === 0) {
    const mode = staged.length > 0 ? "staged" : "local";
    return { mode, ticked: listed(mode) };
  }
  const pending = new Set(unstaged.map((file) => file.path));
  const mode = paths.some((path) => pending.has(path)) ? "local" : "staged";
  const chosen = new Set(paths);
  return { mode, ticked: listed(mode).filter((path) => chosen.has(path)) };
}

export function draftKeyOf(root: string): string {
  return `cogit:draft:${root}`;
}

/** Local changes are the union of both lists: a file staged in part is in both, once. */
export function listedFiles(
  mode: CommitMode,
  staged: readonly FileEntry[],
  unstaged: readonly FileEntry[],
): FileEntry[] {
  if (mode === "staged") return [...staged];
  const seen = new Set(staged.map((file) => file.path));
  return [...staged, ...unstaged.filter((file) => !seen.has(file.path))];
}

/** Which comparison a listed row's State is read from: the index in staged mode; in local
    mode a file also changed on disk after staging reads as its unstaged side. */
export function stateSide(
  mode: CommitMode,
  path: string,
  staged: readonly FileEntry[],
  unstaged: readonly FileEntry[],
): StateSide {
  if (mode === "staged") return "index";
  const inIndex = staged.some((file) => file.path === path);
  return inIndex && !unstaged.some((file) => file.path === path) ? "index" : "worktree";
}

export function splitPath(path: string): { name: string; directory: string } {
  const at = path.lastIndexOf("/");
  return at < 0 ? { name: path, directory: "" } : { name: path.slice(at + 1), directory: path.slice(0, at) };
}

export type SortKey = "name" | "directory";

export function sortedFiles(files: readonly FileEntry[], key: SortKey, descending: boolean): FileEntry[] {
  const part = (file: FileEntry) => splitPath(file.path)[key];
  const sign = descending ? -1 : 1;
  return [...files].sort(
    (a, b) =>
      sign * part(a).localeCompare(part(b), undefined, { sensitivity: "base" }) || a.path.localeCompare(b.path),
  );
}

/** `2 files (+1 ~1)`: how many are selected, then how they changed: `+` new, `~` modified,
    `-` deleted, `>` renamed. */
export function counter(selected: readonly FileEntry[]): string {
  if (selected.length === 0) return "0 files";
  const tally = { "+": 0, "~": 0, "-": 0, ">": 0 };
  for (const file of selected) {
    if (file.status === "added" || file.status === "untracked" || file.status === "copied") tally["+"] += 1;
    else if (file.status === "deleted") tally["-"] += 1;
    else if (file.status === "renamed") tally[">"] += 1;
    else tally["~"] += 1;
  }
  const parts = Object.entries(tally)
    .filter(([, count]) => count > 0)
    .map(([mark, count]) => `${mark}${count}`);
  return `${selected.length} ${selected.length === 1 ? "file" : "files"} (${parts.join(" ")})`;
}

export const COUNTER_INFO =
  "Selected files, then how they changed: + new or untracked, ~ modified, - deleted, > renamed.";

export interface CommitPlan {
  /** Stage these first: Local Changes commits files that are not in the index yet. */
  stage: string[];
  /** The paths the commit is narrowed to; empty means everything staged. */
  only: string[];
}

/** Staged mode narrows the commit to the ticked files unless they are all of the staged
    ones; local mode stages what is not staged yet and commits exactly the ticked files. */
export function planCommit(
  mode: CommitMode,
  ticked: readonly string[],
  staged: readonly FileEntry[],
  unstaged: readonly FileEntry[],
): CommitPlan {
  if (mode === "staged") {
    return { stage: [], only: ticked.length === staged.length ? [] : [...ticked] };
  }
  const changed = new Set(unstaged.map((file) => file.path));
  return { stage: ticked.filter((path) => changed.has(path)), only: [...ticked] };
}

export interface ReadyState {
  message: string;
  template: string | null;
  ticked: number;
  stagedTotal: number;
  amend: boolean;
  unborn: boolean;
  busy: boolean;
  committing: boolean;
  conflicted: number;
}

/** Why Commit holds, shown on it; `null` when it may go. Amend may reword with no file
    ticked, but only when nothing is staged: with staged files, an empty selection would
    commit all of them. */
export function readyBlocker(state: ReadyState): string | null {
  if (state.conflicted > 0) return "Resolve the conflicts first: a ticked file still has conflict markers";
  const rewordOnly = state.amend && !state.unborn && state.ticked === 0;
  const box = {
    message: state.message,
    template: state.template,
    stagedCount: rewordOnly ? 1 : state.ticked,
    amend: state.amend,
    busy: state.busy,
    committing: state.committing,
    scopeEmpty: false,
    unborn: state.unborn,
  };
  const blocker = commitBlocker(box, "Tick the files to commit");
  if (blocker !== null || !rewordOnly || state.stagedTotal === 0) return blocker;
  return "Tick the files to amend with: with files staged, Amend cannot change the message alone";
}

export function ready(state: ReadyState): boolean {
  return readyBlocker(state) === null;
}

/** Paths whose changes the commit leaves out, whole or in part: the unticked ones, and in
    staged mode every file with changes not staged, ticked or not. */
export function leftOut(
  mode: CommitMode,
  ticked: ReadonlySet<string>,
  staged: readonly FileEntry[],
  unstaged: readonly FileEntry[],
): string[] {
  const out = listedFiles(mode, staged, unstaged)
    .map((file) => file.path)
    .filter((path) => !ticked.has(path));
  if (mode === "local") return out;
  const seen = new Set(out);
  return [...out, ...unstaged.map((file) => file.path).filter((path) => !seen.has(path))];
}

/** Asked before a commit that leaves changes out (`leftOut`). */
export function partialQuestion(paths: readonly string[], push: boolean) {
  const files = paths.length === 1 ? "1 file" : `${paths.length} files`;
  return {
    title: "Commit Part of the Changes?",
    message: `Changes in ${files} stay out of this commit: not ticked, or not staged. They remain in the working tree as they are.`,
    confirm: push ? "Commit & Push" : "Commit",
    items: [...paths],
  };
}

export const RECENT_MESSAGES = 10;

/** A commit's whole message as it was written. */
export function fullMessage(details: { summary: string; body: string }): string {
  return details.body === "" ? details.summary : `${details.summary}\n\n${details.body}`;
}

/** The messages worth offering again: each distinct one once, newest first. */
export function recentMessages(
  commits: readonly { summary: string; body: string }[],
  limit = RECENT_MESSAGES,
): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const commit of commits) {
    const message = fullMessage(commit);
    if (message.trim() === "" || seen.has(message)) continue;
    seen.add(message);
    out.push(message);
    if (out.length === limit) break;
  }
  return out;
}

/** A menu row's text: the subject, cut for the row. */
export function menuLabel(message: string, max = 60): string {
  const subject = message.split("\n", 1)[0] ?? "";
  return [...subject].length > max ? `${[...subject].slice(0, max - 1).join("")}…` : subject;
}

/** What ticking Amend does to the field: it takes the last commit's message and remembers
    what was typed; unticking gives that back, unless the message was edited meanwhile. */
export function amendMessage(
  ticked: boolean,
  current: string,
  last: string,
  before: string | null,
): { message: string; before: string | null } {
  if (ticked) return { message: last, before: current };
  return current === last && before !== null
    ? { message: before, before: null }
    : { message: current, before: null };
}
