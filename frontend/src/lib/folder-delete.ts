import type { RefDeletionKind, RefDeletionReport } from "$lib/ipc";
import type { RefNode } from "./ref-nodes";

export interface FolderDeletion {
  kind: RefDeletionKind;
  /** Set for `remoteBranch`: the remote the names below are written under. */
  remote: string | null;
  /** What goes, by ref name (`origin/topic` for a remote branch). */
  names: string[];
  /** Left out because a single delete would refuse it, with the reason. */
  skipped: { name: string; reason: string }[];
}

/** Every ref under a folder row: the rows after it that are deeper than it. The owning
    heading tells the kind. Null for folders of refs that cannot be deleted here. */
export function folderDeletion(tree: readonly RefNode[], folder: RefNode): FolderDeletion | null {
  const at = tree.findIndex((node) => node.id === folder.id);
  if (at < 0 || folder.kind !== "folder") return null;
  const heading = tree.slice(0, at).findLast((node) => node.depth === 0);
  const kind: RefDeletionKind | null =
    heading?.id === "group:local"
      ? "branch"
      : heading?.id === "group:tags"
        ? "tag"
        : heading?.remote !== undefined
          ? "remoteBranch"
          : null;
  if (kind === null) return null;

  const found: FolderDeletion = { kind, remote: heading?.remote ?? null, names: [], skipped: [] };
  for (const node of tree.slice(at + 1)) {
    if (node.depth <= folder.depth) break;
    if (kind === "branch" && node.kind === "local" && node.branch) {
      const name = node.branch.name;
      if (node.current) found.skipped.push({ name, reason: "it is the current branch" });
      else if (node.worktree) {
        found.skipped.push({ name, reason: "it is checked out in another worktree" });
      } else found.names.push(name);
    } else if (kind === "remoteBranch" && node.kind === "remote" && node.branch) {
      // `origin/HEAD` is a pointer to a branch, not a branch of its own.
      if (node.label !== "HEAD") found.names.push(node.branch.name);
    } else if (kind === "tag" && node.kind === "tag" && node.tag) found.names.push(node.tag.name);
  }
  return found;
}

const NOUN = { branch: ["branch", "branches"], remoteBranch: ["remote branch", "remote branches"], tag: ["tag", "tags"] } as const;

export function deletionQuestion(folder: string, plan: FolderDeletion): string {
  const count = plan.names.length;
  const what = `${count} ${NOUN[plan.kind][count === 1 ? 0 : 1]}`;
  const tail =
    plan.kind === "remoteBranch"
      ? `This runs on ${plan.remote ?? "the server"}, and Undo cannot reach it.`
      : "Undo can bring each back.";
  const left =
    plan.skipped.length === 0
      ? ""
      : `\n\nNot included (${plan.skipped.length}): ` +
        plan.skipped.map((each) => `${each.name}, ${each.reason}`).join("; ") +
        ".";
  return `Delete ${what} under ${folder}? ${tail}${left}`;
}

/** What was left after a run, for the follow-up questions and the notifications. */
export function unfinished(report: RefDeletionReport): string[] {
  return [...report.notFullyMerged, ...report.failed.map((each) => each.name)];
}
