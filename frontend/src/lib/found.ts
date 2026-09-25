import type { Found } from "./ipc";

export type FoundStep = { kind: "reveal"; oid: string } | { kind: "file"; path: string } | null;

/** Find Object goes to what was picked and changes nothing (T8.2): a branch or a tag is
    its commit selected and centred in the graph, never a checkout. A file is one of the
    selected commit's. */
export function foundStep(item: Found, commitSelected: boolean): FoundStep {
  if (item.kind === "file") return commitSelected ? { kind: "file", path: item.label } : null;
  return item.oid ? { kind: "reveal", oid: item.oid } : null;
}
