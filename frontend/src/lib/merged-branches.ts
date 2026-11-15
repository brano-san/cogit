import type { BranchMerge, MergeKind } from "$lib/ipc";

/** The gray words beside a local branch HEAD already has (F-949). */
export function mergeMark(kind: MergeKind): string | undefined {
  switch (kind) {
    case "merged":
      return "merged";
    case "squash":
      return "merged by squash";
    case "rebase":
      return "merged by rebase";
    case "notChecked":
      return "merge not checked";
    case "notMerged":
      return undefined;
  }
}

export function mergeHint(kind: MergeKind): string | undefined {
  switch (kind) {
    case "merged":
      return "HEAD contains this branch.";
    case "squash":
      return "One commit in HEAD holds this branch's whole change (squash merge). Deleting it needs force.";
    case "rebase":
      return "Every commit of this branch is in HEAD as an equivalent commit (rebase merge). Deleting it needs force.";
    case "notChecked":
      return "Too many commits since this branch and HEAD split to compare their patches.";
    case "notMerged":
      return undefined;
  }
}

/** Marks by branch name; an answer for a tip the branch has since left is dropped. */
export function marksByName(
  merges: readonly BranchMerge[],
  tips: ReadonlyMap<string, string>,
): ReadonlyMap<string, MergeKind> {
  const marks = new Map<string, MergeKind>();
  for (const merge of merges) {
    if (tips.get(merge.name) === merge.oid) marks.set(merge.name, merge.kind);
  }
  return marks;
}

/** The toast line after Delete Merged Branches after Pull; `null` when nothing went. */
export function deletedMergedText(deleted: readonly BranchMerge[]): string | null {
  if (deleted.length === 0) return null;
  const named = deleted.map((branch) => {
    const how = branch.kind === "merged" ? undefined : mergeMark(branch.kind);
    return how ? `${branch.name} (${how})` : branch.name;
  });
  const noun = deleted.length === 1 ? "merged branch" : "merged branches";
  return `Deleted ${noun}: ${named.join(", ")}`;
}
