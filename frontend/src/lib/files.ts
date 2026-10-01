import type { FileStatus } from "$lib/ipc";

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
  sparse: "◌",
};

export function statusBadge(status: FileStatus): string {
  return BADGES[status];
}

export interface StatusFile {
  status: FileStatus;
  similarity?: number | null;
  indexState?: "staged" | "partly";
  submodule?: { newCommits: boolean; modified: boolean; untracked: boolean } | null;
}

export function isRenamedModified(file: StatusFile): boolean {
  if (file.status !== "renamed") return false;
  if (file.indexState === "partly") return true;
  if (file.similarity !== null && file.similarity !== undefined && file.similarity < 100) return true;
  return false;
}

export function fileStatusBadge(file: StatusFile): string {
  if (isRenamedModified(file)) return "RM";
  return statusBadge(file.status);
}

const LABELS: Partial<Record<FileStatus, string>> = {
  assumeUnchanged: "Assume unchanged",
  sparse: "Outside sparse checkout",
};

export function statusLabel(status: FileStatus): string {
  return LABELS[status] ?? status.charAt(0).toUpperCase() + status.slice(1);
}

export function fileStatusLabel(file: StatusFile): string {
  if (isRenamedModified(file)) return "Renamed and modified";
  return statusLabel(file.status);
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
  sparse: "Outside sparse checkout — hidden by the sparse patterns, not deleted",
};

/** The full name of a status marker and what it means, for its tooltip (R-181). */
export function statusTooltip(status: FileStatus): string {
  return TOOLTIPS[status];
}

export function fileStatusTooltip(file: StatusFile): string {
  if (file.submodule) return submoduleTooltip(file.submodule);
  if (isRenamedModified(file)) {
    return "Renamed and modified — changed since last commit";
  }
  return statusTooltip(file.status);
}

/** What the tooltip adds for a row of the one working-tree list (#32). */
export function indexNote(state: "staged" | "partly" | undefined): string {
  if (state === "staged") return " — staged";
  if (state === "partly") return " — partly staged";
  return "";
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

export function globToRegExp(pattern: string): RegExp {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${escaped.replace(/\*/g, ".*").replace(/\?/g, ".")}$`);
}

/** `git add` records the submodule's current commit only; edits inside it never go with it. */
export function submoduleTooltip(change: NonNullable<StatusFile["submodule"]>): string {
  const inside = [change.modified && "modified files", change.untracked && "untracked files"].filter(Boolean);
  const parts = [
    change.newCommits ? "checked-out commit differs from the one the parent records" : null,
    inside.length ? `${inside.join(" and ")} inside` : null,
  ].filter(Boolean);
  const note = change.newCommits
    ? "Staging records the submodule's current commit; changes inside the submodule are not included."
    : "Nothing to stage: the recorded commit is unchanged, and changes inside the submodule cannot be staged from here. Commit them in the submodule.";
  return `Submodule — ${parts.join("; ")}. ${note}`;
}
