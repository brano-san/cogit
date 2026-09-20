import type { ContextItem } from "./ipc";

const SEPARATOR: ContextItem = { id: "", label: "", enabled: false, separator: true };

function item(id: string, label: string, enabled = true): ContextItem {
  return { id, label, enabled, separator: false };
}

/** A published commit can still be rewritten; the label says what it will cost. */
export function commitMenu(at: { onRemote: boolean }): ContextItem[] {
  const cost = at.onRemote ? " (needs a force-push)" : "";
  return [
    item("cherry-pick", "Cherry-pick onto the current branch"),
    item("revert", "Revert this commit"),
    SEPARATOR,
    item("split-off", `Split off files…${cost}`),
    item("rebase-i", `Rebase commits after this one…${cost}`),
    item("rollback", "Roll back the tree to this commit"),
    SEPARATOR,
    item("copy-sha", "Copy the full SHA"),
  ];
}

export function fileMenu(at: { staged: boolean }): ContextItem[] {
  return [
    at.staged ? item("unstage", "Unstage this file") : item("stage", "Stage this file"),
    ...(at.staged ? [] : [item("discard", "Discard the changes in this file")]),
    SEPARATOR,
    item("blame", "Blame this file"),
    item("copy-path", "Copy the path"),
  ];
}

export function branchMenu(at: { isHead: boolean; hasUpstream: boolean }): ContextItem[] {
  return [
    item("checkout", "Check out this branch", !at.isHead),
    SEPARATOR,
    item("pull", "Pull", at.isHead && at.hasUpstream),
    item("push", "Push", at.isHead),
    SEPARATOR,
    item("merge-branch", "Merge into the current branch", !at.isHead),
    item("rebase-branch", "Rebase the current branch onto this one", !at.isHead),
    SEPARATOR,
    item("delete-branch", "Delete this branch", !at.isHead),
  ];
}
