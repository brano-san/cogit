import { shortOid } from "./format";
import type { Branch, Tag } from "./ipc";
import { localNameOf, tagNameOf, type RefTarget } from "./ref-menus";
import type { RefNode } from "./ref-nodes";

export interface NodeTarget {
  ref: RefTarget | null;
  branch: Branch | null;
  tag?: Tag | null;
  /** The commit it points at; null for a tag on a tree or a blob. */
  oid: string | null;
}

/** The ref a Branches row stands for, as its menu and its double click act on it. */
export function nodeTarget(node: RefNode, tags: readonly Tag[]): NodeTarget | null {
  if (node.kind === "tag") {
    const tag = node.tag ?? tags.find((entry) => entry.name === tagNameOf(node));
    if (!tag) return null;
    return {
      ref: { kind: "tag", name: tag.name, isHead: false },
      branch: null,
      tag,
      oid: tag.pointsToCommit ? tag.oid : null,
    };
  }
  const branch = node.branch;
  if (!branch || (node.kind !== "local" && node.kind !== "remote")) return null;
  return {
    ref: { kind: node.kind === "local" ? "branch" : "remote", name: branch.name, isHead: branch.isHead },
    branch,
    tag: null,
    oid: node.oid ?? null,
  };
}

export type CheckoutPlan =
  | { kind: "switch"; branch: Branch }
  /** HEAD detaches, which the user is asked about first; `what` names it in the question. */
  | { kind: "detach"; oid: string; what: string }
  | null;

/** What Check Out does, from the menu or a double click in Branches alike: a branch is
    switched to — a remote one as its local branch — anything else detaches HEAD. */
export function checkoutPlan(at: NodeTarget, remotes: readonly string[]): CheckoutPlan {
  if (at.ref?.kind === "branch" && at.branch) return { kind: "switch", branch: at.branch };
  if (at.ref?.kind === "remote" && at.branch) {
    return { kind: "switch", branch: { ...at.branch, kind: "local", name: localNameOf(at.branch.name, remotes) } };
  }
  if (!at.oid) return null;
  const what = at.ref?.kind === "tag" ? `tag ${at.ref.name}` : `commit ${shortOid(at.oid)}`;
  return { kind: "detach", oid: at.oid, what };
}
