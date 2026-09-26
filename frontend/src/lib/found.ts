import type { Found } from "./ipc";

export type FoundStep =
  | { kind: "reveal"; oid: string }
  | { kind: "file"; path: string }
  | { kind: "worktree"; path: string }
  | { kind: "staged"; path: string }
  | null;

/** Find Object goes to what was picked and changes nothing (T8.2): a branch or a tag is
    its commit selected and centred in the graph, never a checkout. A file is one of the
    selected commit's, or on the Working Tree its diff in the list that has it: the staged
    one when only the index changed it. */
export function foundStep(
  item: Found,
  commitSelected: boolean,
  stagedOnly: (path: string) => boolean = () => false,
): FoundStep {
  if (item.kind === "file") {
    if (commitSelected) return { kind: "file", path: item.label };
    return { kind: stagedOnly(item.label) ? "staged" : "worktree", path: item.label };
  }
  return item.oid ? { kind: "reveal", oid: item.oid } : null;
}
