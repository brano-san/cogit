import type { FileEntry, FileStatus } from "$lib/ipc";

export type SortKey = "status" | "name" | "path";

const BADGES: Record<FileStatus, string> = {
  added: "A",
  modified: "M",
  deleted: "D",
  renamed: "R",
  copied: "C",
  untracked: "?",
  conflicted: "U",
  unchanged: "·",
  ignored: "∅",
  assumeUnchanged: "≈",
  skipped: "⤳",
};

const STATUS_ORDER: FileStatus[] = [
  "conflicted",
  "added",
  "modified",
  "deleted",
  "renamed",
  "copied",
  "untracked",
  "assumeUnchanged",
  "skipped",
  "ignored",
  "unchanged",
];

export function statusBadge(status: FileStatus): string {
  return BADGES[status];
}

const LABELS: Partial<Record<FileStatus, string>> = {
  assumeUnchanged: "Assume unchanged",
};

export function statusLabel(status: FileStatus): string {
  return LABELS[status] ?? status.charAt(0).toUpperCase() + status.slice(1);
}

const TOOLTIPS: Record<FileStatus, string> = {
  modified: "Modified — changed since last commit",
  added: "Added — new file in the index",
  deleted: "Deleted",
  renamed: "Renamed",
  copied: "Copied",
  untracked: "Untracked — not under version control",
  ignored: "Ignored",
  conflicted: "Conflict — unmerged",
  unchanged: "Unchanged — the same as in the last commit",
  assumeUnchanged: "Assume unchanged — Git does not check this file for changes",
  skipped: "Skip worktree — left out of the working tree",
};

/** The full name of a status marker and what it means, for its tooltip (R-181). */
export function statusTooltip(status: FileStatus): string {
  return TOOLTIPS[status];
}

export function fileName(path: string): string {
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  return trimmed.slice(trimmed.lastIndexOf("/") + 1);
}

export function matchesMask(path: string, mask: string): boolean {
  const pattern = mask.trim().toLowerCase();
  if (pattern === "") return true;

  if (!pattern.includes("*") && !pattern.includes("?")) {
    return path.toLowerCase().includes(pattern);
  }
  const subject = pattern.includes("/") ? path : fileName(path);
  return globToRegExp(pattern).test(subject.toLowerCase());
}

export function sortFiles(files: readonly FileEntry[], key: SortKey): FileEntry[] {
  const byPath = (a: FileEntry, b: FileEntry) => a.path.localeCompare(b.path);
  const compare: Record<SortKey, (a: FileEntry, b: FileEntry) => number> = {
    path: byPath,
    name: (a, b) => fileName(a.path).localeCompare(fileName(b.path)) || byPath(a, b),
    status: (a, b) => rank(a.status) - rank(b.status) || byPath(a, b),
  };
  return [...files].sort(compare[key]);
}

function rank(status: FileStatus): number {
  const index = STATUS_ORDER.indexOf(status);
  return index === -1 ? STATUS_ORDER.length : index;
}

function globToRegExp(pattern: string): RegExp {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${escaped.replace(/\*/g, ".*").replace(/\?/g, ".")}$`);
}
