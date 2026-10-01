import type { ContextItem } from "./ipc";
import { offer } from "./context-menu";
import { groupVariant } from "./group-menu";
import type { FolderDeletion } from "./folder-delete";
import { leavesUnder, type RefNode } from "./ref-nodes";
import { REF_MENU_PREFIX, branchesBranchMenu, graphCommitMenu, type CommitFacts } from "./ref-menus";
import { splitUpstream } from "./push-to";

/** Items of the menu of several rows (graph commits or Branches rows). */
export const SELECTION_MENU_PREFIX = "selection:";

const id = (name: string) => `${SELECTION_MENU_PREFIX}${name}`;
const single = (name: string) => `${REF_MENU_PREFIX}${name}`;

/** The selected rows in the order Branches draws them. */
export function selectedNodes(order: readonly RefNode[], ids: ReadonlySet<string>): RefNode[] {
  return order.filter((node) => ids.has(node.id));
}

/** Space on a selection: every box off when all are on, else every box on. One new set. */
export function toggleSelected(
  tree: readonly RefNode[],
  ids: readonly string[],
  visible: ReadonlySet<string>,
): Set<string> {
  const leaves = [...new Set(ids.flatMap((each) => leavesUnder(tree, each)))];
  const next = new Set(visible);
  if (leaves.length === 0) return next;
  const allOn = leaves.every((leaf) => visible.has(leaf));
  for (const leaf of leaves) {
    if (allOn) next.delete(leaf);
    else next.add(leaf);
  }
  return next;
}

export interface RefPicks {
  locals: RefNode[];
  remotes: RefNode[];
  tags: RefNode[];
}

/** The refs among the selected rows; headings, folders, stashes and HEAD are not refs to
    delete, push or copy. `origin/HEAD` is a pointer, not a branch. */
export function refPicks(nodes: readonly RefNode[]): RefPicks {
  return {
    locals: nodes.filter((node) => node.kind === "local" && node.branch !== undefined),
    remotes: nodes.filter((node) => node.kind === "remote" && node.branch !== undefined && node.label !== "HEAD"),
    tags: nodes.filter((node) => node.kind === "tag" && node.tag !== undefined),
  };
}

/** What Delete takes of the selection: the current branch and those checked out in another
    worktree are left out with the reason, remote branches go by their remote. */
export function selectionDeletion(nodes: readonly RefNode[], remotes: readonly string[]): FolderDeletion[] {
  const picks = refPicks(nodes);
  const plans: FolderDeletion[] = [];

  const locals: FolderDeletion = { kind: "branch", remote: null, names: [], skipped: [] };
  for (const node of picks.locals) {
    const name = (node.branch as { name: string }).name;
    if (node.current) locals.skipped.push({ name, reason: "it is the current branch" });
    else if (node.worktree) locals.skipped.push({ name, reason: "it is checked out in another worktree" });
    else locals.names.push(name);
  }
  if (locals.names.length + locals.skipped.length > 0) plans.push(locals);

  const byRemote = new Map<string, string[]>();
  for (const node of picks.remotes) {
    const name = (node.branch as { name: string }).name;
    const remote = splitUpstream(name, remotes)?.remote ?? name.split("/")[0] ?? name;
    byRemote.set(remote, [...(byRemote.get(remote) ?? []), name]);
  }
  for (const [remote, names] of byRemote) plans.push({ kind: "remoteBranch", remote, names, skipped: [] });

  const tags = picks.tags.map((node) => (node.tag as { name: string }).name);
  if (tags.length > 0) plans.push({ kind: "tag", remote: null, names: tags, skipped: [] });
  return plans;
}

const NOUN = {
  branch: ["branch", "branches"],
  remoteBranch: ["remote branch", "remote branches"],
  tag: ["tag", "tags"],
} as const;

export function selectionQuestion(plans: readonly FolderDeletion[]): string {
  const total = (kind: FolderDeletion["kind"]) =>
    plans.filter((plan) => plan.kind === kind).reduce((sum, plan) => sum + plan.names.length, 0);
  const parts = (["branch", "remoteBranch", "tag"] as const)
    .map((kind) => [kind, total(kind)] as const)
    .filter(([, count]) => count > 0)
    .map(([kind, count]) => `${count} ${NOUN[kind][count === 1 ? 0 : 1]}`);
  const lines = [`Delete ${parts.join(", ")}?`];
  if (total("branch") + total("tag") > 0) lines.push("Undo can bring back the local branches and tags.");
  if (total("remoteBranch") > 0) {
    lines.push("Remote branches are deleted on their servers (git push --delete), and Undo cannot reach them.");
  }
  const skipped = plans.flatMap((plan) => plan.skipped);
  if (skipped.length > 0) {
    lines.push(`Not included (${skipped.length}): ${skipped.map((each) => `${each.name}, ${each.reason}`).join("; ")}.`);
  }
  return lines.join("\n\n");
}

export function selectionDeletable(plans: readonly FolderDeletion[]): boolean {
  return plans.some((plan) => plan.names.length > 0);
}

const NEUTRAL: CommitFacts = {
  isHeadCommit: false,
  detachedHere: false,
  onHead: false,
  published: false,
  parents: 1,
  upstream: null,
  hasRemote: false,
};

export interface BranchSelectionFacts {
  picks: RefPicks;
  hasRemote: boolean;
  deletable: boolean;
  /** Why no box of the selection can change, or null. */
  toggle: string | null;
}

/** Several rows of Branches: Push To…, Delete…, Copy and Toggle act on all of them, every
    other item of the single menu stays and is off. */
export function branchSelectionMenu(facts: BranchSelectionFacts): ContextItem[] {
  const template = branchesBranchMenu({ kind: "branch", name: "", isHead: false }, NEUTRAL, {
    selected: null,
    oid: null,
    untickable: null,
  });
  const refs = facts.picks.locals.length + facts.picks.remotes.length + facts.picks.tags.length;
  const live = new Map<string, ContextItem>([
    [
      single("push-to"),
      offer(
        id("push-to"),
        "Push To…",
        facts.picks.locals.length === 0 ? "no local branch selected" : facts.hasRemote ? null : "no remote",
      ),
    ],
    [single("delete"), offer(id("delete"), "Delete…", facts.deletable ? null : "nothing here can be deleted")],
    [single("copy-name"), offer(id("copy"), "Copy", refs > 0 ? null : "no branch or tag selected")],
    [single("toggle"), offer(id("toggle"), "Toggle", facts.toggle, "Space")],
  ]);
  return groupVariant(template, live, "several selected");
}

/** Several commits of the graph: Cherry-Pick…, Revert…, Copy Message and Copy ID. */
export function commitSelectionMenu(): ContextItem[] {
  const template = graphCommitMenu(NEUTRAL);
  const live = new Map<string, ContextItem>([
    [single("cherry-pick"), offer(id("cherry-pick"), "Cherry-Pick…", null)],
    [single("revert"), offer(id("revert"), "Revert…", null)],
    [single("copy-message"), offer(id("copy-message"), "Copy Message", null)],
    [single("copy-id"), offer(id("copy-id"), "Copy ID", null)],
  ]);
  return groupVariant(template, live, "several selected");
}

/** Cherry-pick takes the oldest first, revert the newest first; `rows` are graph rows,
    where a larger row is older. Click order never matters. */
export function replayOrder(
  rows: ReadonlyMap<string, number>,
  oids: readonly string[],
  direction: "oldest-first" | "newest-first",
): string[] {
  const sign = direction === "oldest-first" ? -1 : 1;
  return [...oids].sort((a, b) => sign * ((rows.get(a) ?? 0) - (rows.get(b) ?? 0)));
}
