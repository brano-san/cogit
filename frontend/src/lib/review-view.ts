import type { BranchReview } from "$lib/ipc/review";
import type { ChangeKind } from "$lib/ipc";
import { shortOid } from "$lib/format";
import { branchName } from "$lib/review-params";

export function baseOptions(
  branches: readonly string[],
  query: string,
): string[] {
  const search = query.trim().toLowerCase();
  return branches.filter((ref) => !isRemoteHead(ref) && branchName(ref).toLowerCase().includes(search));
}

function isRemoteHead(ref: string): boolean {
  return ref.startsWith("refs/remotes/") && ref.endsWith("/HEAD");
}

export function baseCaption(summary: BranchReview): string {
  const bases = summary.mergeBases.map(shortOid).join(", ");
  return [
    `${branchName(summary.base)} at ${shortOid(summary.baseOid)}, ${branchName(summary.branch)} at ${shortOid(summary.branchOid)}`,
    `Merge base ${shortOid(summary.mergeBase)} · ${summary.mergeBaseSummary}`,
    `git diff ${branchName(summary.base)}...${branchName(summary.branch)}`,
    summary.mergeBases.length > 1 ? `Merge bases: ${bases}` : null,
  ]
    .filter((line) => line !== null)
    .join("\n");
}

export function reloadsOn(kind: ChangeKind, worktree: boolean): boolean {
  if (kind === "head" || kind === "refs") return true;
  return worktree && (kind === "workingTree" || kind === "index");
}
