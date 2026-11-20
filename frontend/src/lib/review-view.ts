import type { BranchReview } from "$lib/ipc/review";
import type { ChangeKind } from "$lib/ipc";
import { shortOid } from "$lib/format";

export const REVIEW_SHOWN_COMMITS = 2000;

export function baseOptions(
  branches: readonly string[],
  branch: string,
  current: string | null,
): (readonly [string, string])[] {
  const names = branches.filter((name) => name !== branch && !name.endsWith("/HEAD"));
  if (current && !names.includes(current)) names.unshift(current);
  if (names.length === 0) return [["", "No other branch"]];
  return current ? names.map((name) => [name, name] as const) : [["", "Choose a base"], ...names.map((name) => [name, name] as const)];
}

export function baseCaption(summary: BranchReview): string {
  const bases = summary.mergeBases.map(shortOid).join(", ");
  return [
    `${summary.base} at ${shortOid(summary.baseOid)}, ${summary.branch} at ${shortOid(summary.branchOid)}`,
    `Changes since the merge base ${shortOid(summary.mergeBase)} (git diff ${summary.base}...${summary.branch})`,
    summary.mergeBases.length > 1 ? `Merge bases: ${bases}` : null,
  ]
    .filter((line) => line !== null)
    .join("\n");
}

export function reloadsOn(kind: ChangeKind, worktree: boolean): boolean {
  if (kind === "head" || kind === "refs") return true;
  return worktree && (kind === "workingTree" || kind === "index");
}
