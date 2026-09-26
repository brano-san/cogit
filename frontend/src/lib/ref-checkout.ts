import { shortOid } from "./format";
import type { Branch, CheckoutTarget, Head, Tag } from "./ipc";
import { branchNameProblem } from "./names";
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
  if (node.kind === "remote") {
    return { ref: { kind: "remote", name: branch.name, isHead: branch.isHead }, branch, tag: null, oid: node.oid ?? null };
  }
  const ref: RefTarget = { kind: "branch", name: branch.name, isHead: branch.isHead, upstream: branch.upstream };
  return {
    ref: node.worktree ? { ...ref, worktree: node.worktree.path } : ref,
    branch,
    tag: null,
    oid: node.oid ?? null,
  };
}

/** What a double click on a Branches row does (item 40); HEAD does nothing. */
export type RefActivation = "fold" | "checkout" | "apply-stash" | "recover" | null;

export function refActivation(node: RefNode): RefActivation {
  if (node.children === true) return "fold";
  switch (node.kind) {
    case "local":
    case "remote":
    case "tag":
      return "checkout";
    case "stash":
      return "apply-stash";
    case "lost":
      return node.oid ? "recover" : null;
    default:
      return null;
  }
}

export type CheckoutChoice = "create" | "detach" | "local";

/** An existing local branch among the choices: the one that tracks the remote branch, or
    the branch whose label was double-clicked in the graph. */
export interface LocalChoice {
  name: string;
  /** Moved forward to `to` first (`fastForward`); null checks it out as it is. */
  to: string | null;
  label: string;
  explanation: string;
  blocked: string | null;
}

/** The one Checkout dialog (item 40): a local branch checked out as it is, which the
    dialog only describes, or up to three ways to get to a commit. */
export interface CheckoutOffer {
  /** What HEAD goes to, as the dialog's first line names it. */
  source: string;
  /** A local branch as it is: the dialog says what happens and offers Don't show again. */
  plain: Branch | null;
  /** A new local branch: the name it suggests, the start git is given, and the remote
      branch Track remote branch follows — null where there is nothing to track. */
  create: { name: string; start: string; remote: string | null } | null;
  detach: { oid: string; blocked: string | null } | null;
  local: LocalChoice | null;
  initial: CheckoutChoice;
}

export interface CheckoutContext {
  branches: readonly Branch[];
  remotes: readonly string[];
  head: Head | null | undefined;
}

/** Branches checks a local branch out as it is; the graph offers the three choices, as
    for a remote branch with a local one tracking it (item 40, R-561). */
export type CheckoutPlace = "branches" | "graph";

export interface CheckoutPick {
  choice: CheckoutChoice;
  name: string;
  track: boolean;
}

export interface CheckoutRequest {
  target: CheckoutTarget;
  /** The existing local branch HEAD goes to: another worktree may have it checked out. */
  branch: string | null;
  /** How the questions and the errors name where HEAD goes. */
  what: string;
}

const commits = (n: number) => `${n} commit${n === 1 ? "" : "s"}`;

function detachedAt(head: Head | null | undefined, oid: string): string | null {
  return head?.kind === "detached" && head.oid === oid ? "HEAD is already detached here" : null;
}

/** What the dialog says about a local branch it checks out as it is. */
export function branchFacts(branch: Branch): string {
  const { name, upstream, ahead, behind } = branch;
  if (upstream === null) return `${name} tracks no remote branch.`;
  if (ahead === 0 && behind === 0) return `${name} is up to date with ${upstream}.`;
  if (ahead === 0) return `${name} is ${commits(behind)} behind ${upstream}.`;
  if (behind === 0) return `${name} is ${commits(ahead)} ahead of ${upstream}.`;
  return `${name} and ${upstream} have diverged: ↑${ahead} ↓${behind}.`;
}

/** The local branch git links to `remote`: by its upstream, never by name alone (R-560).
    Of several, the one of the same name, else the first by name. */
function trackingLocal(remote: Branch, ctx: CheckoutContext): Branch | null {
  const suggested = localNameOf(remote.name, ctx.remotes);
  const rank = (branch: Branch) => (branch.name === suggested ? 0 : 1);
  const tracking = ctx.branches
    .filter((branch) => branch.kind === "local" && branch.upstream === remote.name)
    .sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
  return tracking[0] ?? null;
}

/** Fast-forward only when there is something to move and nothing of its own in the way;
    otherwise the branch is checked out as it is, and the dialog says why (R-560). */
function trackingChoice(local: Branch, remote: Branch): LocalChoice {
  const { name, ahead, behind } = local;
  if (behind > 0 && ahead === 0) {
    return {
      name,
      to: remote.fullName,
      label: `Checkout and fast-forward local branch '${name}'`,
      explanation: `${name} is ${commits(behind)} behind ${remote.name} and moves forward to it.`,
      blocked: null,
    };
  }
  const explanation =
    behind === 0
      ? `${name} already has every commit of ${remote.name}${ahead > 0 ? ` and ${commits(ahead)} more` : ""}.`
      : `${name} and ${remote.name} have diverged (↑${ahead} ↓${behind}): it is checked out as it is, to merge or rebase afterwards.`;
  return {
    name,
    to: null,
    label: `Check out local branch '${name}'`,
    explanation,
    blocked: local.isHead ? "already checked out" : null,
  };
}

function remoteOffer(remote: Branch, ctx: CheckoutContext): CheckoutOffer {
  const tracking = trackingLocal(remote, ctx);
  const local = tracking ? trackingChoice(tracking, remote) : null;
  return {
    source: remote.name,
    plain: null,
    create: { name: localNameOf(remote.name, ctx.remotes), start: remote.fullName, remote: remote.name },
    detach: { oid: remote.oid, blocked: detachedAt(ctx.head, remote.oid) },
    local,
    initial: local && local.blocked === null ? "local" : "create",
  };
}

/** A commit or a tag: a new branch there, or a look at it with HEAD detached. */
function commitOffer(source: string, oid: string, ctx: CheckoutContext): CheckoutOffer {
  const blocked = detachedAt(ctx.head, oid);
  return {
    source,
    plain: null,
    create: { name: "", start: oid, remote: null },
    detach: { oid, blocked },
    local: null,
    initial: blocked === null ? "detach" : "create",
  };
}

/** A local branch's label in the graph: its commit's two choices and the branch itself. */
function labelOffer(branch: Branch, ctx: CheckoutContext): CheckoutOffer {
  return {
    ...commitOffer(branch.name, branch.oid, ctx),
    local: {
      name: branch.name,
      to: null,
      label: `Check out local branch '${branch.name}'`,
      explanation: branchFacts(branch),
      blocked: null,
    },
    initial: "local",
  };
}

/** What the dialog offers for a row or a label, the menu's Check Out and a double click
    alike; null where there is nothing to check out: HEAD's own branch, a tag on a tree. */
export function checkoutOffer(at: NodeTarget, ctx: CheckoutContext, place: CheckoutPlace): CheckoutOffer | null {
  const ref = at.ref;
  if (ref?.kind === "branch" && at.branch) {
    if (at.branch.isHead) return null;
    if (place === "graph") return labelOffer(at.branch, ctx);
    return { source: at.branch.name, plain: at.branch, create: null, detach: null, local: null, initial: "local" };
  }
  if (ref?.kind === "remote" && at.branch) return remoteOffer(at.branch, ctx);
  if (!at.oid) return null;
  return commitOffer(ref?.kind === "tag" ? `tag ${ref.name}` : `commit ${shortOid(at.oid)}`, at.oid, ctx);
}

/** A local branch of that name is in the way. One that does not track the remote branch
    is not taken for it (R-560): the user picks another name or sets its upstream. */
export function newBranchProblem(name: string, offer: CheckoutOffer, branches: readonly Branch[]): string | null {
  const locals = branches.filter((branch) => branch.kind === "local");
  const trimmed = name.trim();
  const remote = offer.create?.remote ?? null;
  const same = locals.find((branch) => branch.name === trimmed);
  if (same && remote !== null && same.upstream !== remote) {
    return `${trimmed} already exists and does not track ${remote}. Choose another name, or set its upstream to ${remote} first.`;
  }
  return branchNameProblem(name, locals.map((branch) => branch.name));
}

/** Why Checkout is off for this pick, or null. */
export function pickProblem(offer: CheckoutOffer, pick: CheckoutPick, branches: readonly Branch[]): string | null {
  if (offer.plain) return null;
  switch (pick.choice) {
    case "create":
      return offer.create ? newBranchProblem(pick.name, offer, branches) : "nothing to start a branch from";
    case "detach":
      return offer.detach ? offer.detach.blocked : "nothing to detach at";
    case "local":
      return offer.local ? offer.local.blocked : "no local branch to check out";
  }
}

/** What the pick asks git for; null for a pick `pickProblem` refuses. */
export function checkoutRequest(offer: CheckoutOffer, pick: CheckoutPick): CheckoutRequest | null {
  if (offer.plain) {
    const name = offer.plain.name;
    return { target: { kind: "branch", name }, branch: name, what: name };
  }
  if (pick.choice === "create" && offer.create) {
    const name = pick.name.trim();
    if (name === "") return null;
    const track = offer.create.remote !== null && pick.track;
    return { target: { kind: "newBranch", name, start: offer.create.start, track }, branch: null, what: name };
  }
  if (pick.choice === "detach" && offer.detach) {
    return { target: { kind: "commit", oid: offer.detach.oid }, branch: null, what: offer.source };
  }
  if (pick.choice === "local" && offer.local) {
    const { name, to } = offer.local;
    const target: CheckoutTarget = to ? { kind: "fastForward", name, to } : { kind: "branch", name };
    return { target, branch: name, what: name };
  }
  return null;
}
