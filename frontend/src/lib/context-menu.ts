import type { ContextItem } from "./ipc";

const SEPARATOR: ContextItem = { id: "", label: "", enabled: false, separator: true };

function item(id: string, label: string, enabled = true, accelerator?: string): ContextItem {
  return { id, label, enabled, separator: false, accelerator: accelerator ?? null };
}

/** Drops separators that separate nothing, so a menu can be written as a flat list and
    each row can switch itself off (the same rule `menu::tidy` applies in Rust). */
export function tidy(entries: readonly ContextItem[]): ContextItem[] {
  const kept: ContextItem[] = [];
  for (const entry of entries) {
    if (entry.separator && (kept.length === 0 || kept[kept.length - 1]!.separator)) continue;
    kept.push(entry);
  }
  while (kept.length > 0 && kept[kept.length - 1]!.separator) kept.pop();
  return kept;
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
    item("rename-branch", "Rename this branch…"),
    item("set-upstream", "Set the upstream…"),
    item("clear-upstream", "Stop tracking an upstream", at.hasUpstream),
    SEPARATOR,
    item("delete-branch", "Delete this branch", !at.isHead),
  ];
}

/** A remote branch lives on the server: local deletion would be a lie, so it is not offered. */
function remoteBranchMenu(): ContextItem[] {
  return [
    item("checkout", "Check out a local branch from this one"),
    SEPARATOR,
    item("merge-branch", "Merge into the current branch"),
    item("rebase-branch", "Rebase the current branch onto this one"),
    SEPARATOR,
    item("delete-remote-branch", "Delete this branch on the remote…"),
  ];
}

/** One menu for the whole References tree: the node kind decides what is on offer. */
export function refMenu(at: {
  kind: string;
  isHead: boolean;
  hasUpstream: boolean;
}): ContextItem[] {
  switch (at.kind) {
    case "local":
      return branchMenu({ isHead: at.isHead, hasUpstream: at.hasUpstream });
    case "remote":
      return remoteBranchMenu();
    case "tag":
      return [
        item("checkout-tag", "Check out this tag"),
        item("delete-tag", "Delete this tag"),
        SEPARATOR,
        item("copy-sha", "Copy the full SHA"),
      ];
    case "stash":
      return [
        item("apply-stash", "Apply this stash"),
        item("pop-stash", "Pop this stash"),
        SEPARATOR,
        item("drop-stash", "Drop this stash"),
      ];
    case "lost":
      return [
        item("restore-lost", "Create a branch here"),
        SEPARATOR,
        item("copy-sha", "Copy the full SHA"),
      ];
    default:
      return [];
  }
}
