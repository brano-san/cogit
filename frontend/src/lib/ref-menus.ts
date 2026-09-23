import type { Branch, ContextItem, Head, Tag } from "./ipc";
import { SEPARATOR, offer, tidy } from "./context-menu";

/** What the graph and Branches menus need to know about the commit they act on. */
export interface CommitFacts {
  isHeadCommit: boolean;
  detachedHere: boolean;
  /** In HEAD's history, so a rebase from HEAD can reach and rewrite it. */
  onHead: boolean;
  /** Reachable from a remote-tracking branch — `is_published`. */
  published: boolean;
  parents: number;
  /** Where Push Up To sends HEAD's branch: its upstream, e.g. `origin/main`. */
  upstream: string | null;
  hasRemote: boolean;
}

export interface RefTarget {
  kind: "branch" | "remote" | "tag";
  name: string;
  isHead: boolean;
}

export interface WorkingTreeCounts {
  staged: number;
  modified: number;
  untracked: number;
}

export const REF_MENU_PREFIX = "ref:";

const id = (name: string) => `${REF_MENU_PREFIX}${name}`;

const PUSHED = "already pushed";
const IN_HEAD = "already in HEAD";

/** Why history from HEAD cannot be rewritten at this commit, or null when it can. */
function rewriteBlocked(facts: CommitFacts): string | null {
  if (!facts.onHead) return "not on the current branch";
  if (facts.parents > 1) return "a merge commit";
  return null;
}

function squashBlocked(facts: CommitFacts): string | null {
  return (
    rewriteBlocked(facts) ??
    (facts.parents === 0 ? "the first commit" : null) ??
    (facts.published ? PUSHED : null)
  );
}

function replayBlocked(facts: CommitFacts): string | null {
  return facts.parents > 1 ? "a merge commit" : null;
}

function commitActions(facts: CommitFacts, onRef: RefTarget | null): ContextItem[] {
  const current = onRef?.kind === "branch" && onRef.isHead ? "the current branch" : null;
  return [
    offer(id("merge"), "Merge", current ?? (facts.onHead ? IN_HEAD : null)),
    offer(id("cherry-pick"), "Cherry-Pick", (facts.onHead ? IN_HEAD : null) ?? replayBlocked(facts)),
    offer(id("revert"), "Revert", (facts.onHead ? null : "not in HEAD") ?? replayBlocked(facts)),
    offer(id("rebase"), "Rebase", current ?? (facts.onHead ? IN_HEAD : null)),
    SEPARATOR,
    offer(id("modify"), "Modify", rewriteBlocked(facts)),
    offer(id("split"), "Split", rewriteBlocked(facts)),
    offer(id("squash"), "Squash", squashBlocked(facts)),
    offer(id("edit-message"), "Edit Message", rewriteBlocked(facts)),
    offer(id("edit-author"), "Edit Author", rewriteBlocked(facts)),
    offer(
      id("rebase-i"),
      "Rebase Interactive From",
      facts.onHead ? (facts.isHeadCommit ? "nothing after it" : null) : "not on the current branch",
    ),
    SEPARATOR,
    offer(id("add-branch"), "Add Branch", null),
    offer(id("add-tag"), "Add Tag", null),
    SEPARATOR,
    offer(id("reset"), "Reset", facts.isHeadCommit ? "HEAD is already here" : null),
    offer(id("reset-advanced"), "Reset Advanced…", null),
  ];
}

/** #38: a commit row of the graph. */
export function graphCommitMenu(facts: CommitFacts): ContextItem[] {
  return tidy([
    offer(id("checkout"), "Check Out", facts.detachedHere ? "already checked out" : null),
    ...commitActions(facts, null),
    SEPARATOR,
    offer(
      id("push-up-to"),
      "Push Up To",
      !facts.hasRemote
        ? "no remote"
        : facts.upstream === null
          ? "the branch has no upstream"
          : !facts.onHead
            ? "not on the current branch"
            : facts.published
              ? PUSHED
              : null,
    ),
    SEPARATOR,
    offer(id("copy-message"), "Copy Message", null),
    offer(id("copy-id"), "Copy ID", null),
  ]);
}

function checkoutBlocked(ref: RefTarget, facts: CommitFacts): string | null {
  if (ref.kind === "branch") return ref.isHead ? "already checked out" : null;
  if (ref.kind === "tag") return facts.detachedHere ? "already checked out" : null;
  return null;
}

function pushBlocked(ref: RefTarget, facts: CommitFacts): string | null {
  if (ref.kind === "remote") return "a remote branch";
  return facts.hasRemote ? null : "no remote";
}

/** #39: a branch or tag label on a graph row. */
export function graphRefMenu(ref: RefTarget, facts: CommitFacts): ContextItem[] {
  return tidy([
    offer(id("checkout"), "Check Out", checkoutBlocked(ref, facts)),
    ...commitActions(facts, ref),
    SEPARATOR,
    offer(id("push"), "Push", pushBlocked(ref, facts)),
    offer(id("push-to"), "Push To…", pushBlocked(ref, facts)),
    SEPARATOR,
    offer(id("delete"), "Delete", ref.kind === "branch" && ref.isHead ? "checked out" : null),
    offer(
      id("rename"),
      "Rename",
      ref.kind === "remote" ? "a remote branch" : facts.published ? PUSHED : null,
    ),
    SEPARATOR,
    offer(id("copy-name"), "Copy Name", null),
    offer(id("copy-message"), "Copy Message", null),
    offer(id("copy-id"), "Copy ID", null),
  ]);
}

export interface BranchesContext {
  selected: string | null;
  /** The commit the row points at; null for a tag on a tree or a blob. */
  oid: string | null;
  untickable: string | null;
}

function compareRows(at: BranchesContext, facts: CommitFacts): ContextItem[] {
  const none = at.oid === null ? "not a commit" : null;
  return [
    offer(id("reveal"), "Reveal Commit", none ?? at.untickable),
    offer(id("compare-head"), "Compare with HEAD", none ?? (facts.isHeadCommit ? "same as HEAD" : null)),
    offer(
      id("compare-selected"),
      "Compare with Selected Commit",
      none ??
        (at.selected === null
          ? "no commit selected"
          : at.selected === at.oid
            ? "it is the selected one"
            : null),
    ),
  ];
}

function resetRows(at: BranchesContext, facts: CommitFacts): ContextItem[] {
  const none = at.oid === null ? "not a commit" : null;
  return [
    offer(id("reset"), "Reset", none ?? (facts.isHeadCommit ? "HEAD is already here" : null)),
    offer(id("reset-advanced"), "Reset Advanced…", none),
  ];
}

/** #33: a local or remote branch in Branches. */
export function branchesBranchMenu(
  ref: RefTarget,
  facts: CommitFacts,
  at: BranchesContext,
): ContextItem[] {
  const current = ref.isHead ? "the current branch" : null;
  return tidy([
    offer(id("checkout"), "Check Out", checkoutBlocked(ref, facts)),
    SEPARATOR,
    ...compareRows(at, facts),
    SEPARATOR,
    offer(id("merge"), "Merge", current ?? (facts.onHead ? IN_HEAD : null)),
    offer(id("rebase"), "Rebase", current ?? (facts.onHead ? IN_HEAD : null)),
    SEPARATOR,
    offer(id("push"), "Push", pushBlocked(ref, facts)),
    offer(id("push-to"), "Push To…", pushBlocked(ref, facts)),
    SEPARATOR,
    ...resetRows(at, facts),
    SEPARATOR,
    offer(id("delete"), "Delete", ref.isHead ? "checked out" : null),
    SEPARATOR,
    offer(id("copy-name"), "Copy", null),
    SEPARATOR,
    offer(id("toggle"), "Toggle", at.untickable),
  ]);
}

/** #34: a tag in Branches. */
export function branchesTagMenu(
  facts: CommitFacts,
  at: BranchesContext & { annotated: boolean },
): ContextItem[] {
  const none = at.oid === null ? "not a commit" : null;
  return tidy([
    offer(id("checkout"), "Check Out", none ?? (facts.detachedHere ? "already checked out" : null)),
    SEPARATOR,
    ...compareRows(at, facts),
    SEPARATOR,
    offer(id("merge"), "Merge", none ?? (facts.onHead ? IN_HEAD : null)),
    SEPARATOR,
    offer(id("push"), "Push", facts.hasRemote ? null : "no remote"),
    offer(id("push-to"), "Push To…", facts.hasRemote ? null : "no remote"),
    SEPARATOR,
    ...resetRows(at, facts),
    SEPARATOR,
    offer(id("delete"), "Delete", null),
    SEPARATOR,
    offer(id("copy-name"), "Copy", null),
    offer(id("copy-tag-message"), "Copy Message", at.annotated ? null : "a lightweight tag"),
    SEPARATOR,
    offer(id("toggle"), "Toggle", at.untickable),
  ]);
}

/** #35: a stash in Branches. */
export function branchesStashMenu(facts: CommitFacts, at: BranchesContext): ContextItem[] {
  return tidy([
    offer(id("apply-stash"), "Apply Stash", null),
    SEPARATOR,
    ...compareRows(at, facts),
    SEPARATOR,
    offer(id("rename-stash"), "Rename Stash", null),
    SEPARATOR,
    offer(id("drop-stash"), "Drop Stash", null),
    SEPARATOR,
    offer(id("copy-stash-message"), "Copy Message", null),
    SEPARATOR,
    offer(id("toggle"), "Toggle", at.untickable),
  ]);
}

/** #37: the Working Tree row at the top of the graph. */
export function workingTreeMenu(counts: WorkingTreeCounts): ContextItem[] {
  const changed = counts.staged + counts.modified + counts.untracked;
  return tidy([
    offer(id("wt-commit"), "Commit", changed > 0 ? null : "nothing to commit"),
    offer(id("wt-stage"), "Stage", counts.modified + counts.untracked > 0 ? null : "nothing to stage"),
    offer(id("wt-unstage"), "Unstage", counts.staged > 0 ? null : "nothing staged"),
    offer(id("wt-discard"), "Discard", counts.modified > 0 ? null : "nothing to discard"),
  ]);
}

export function commitFacts(input: {
  oid: string;
  parents: number;
  head: Head | null | undefined;
  branches: readonly Branch[];
  onHead: boolean;
  published: boolean;
  hasRemote: boolean;
}): CommitFacts {
  const head = input.head;
  const headOid = head && head.kind !== "unborn" ? head.oid : null;
  const headBranch =
    head?.kind === "branch"
      ? input.branches.find((branch) => branch.kind === "local" && branch.name === head.name)
      : undefined;
  return {
    isHeadCommit: headOid === input.oid,
    detachedHere: head?.kind === "detached" && head.oid === input.oid,
    onHead: input.onHead,
    published: input.published,
    parents: input.parents,
    upstream: headBranch?.upstream ?? null,
    hasRemote: input.hasRemote,
  };
}

/** The ref a graph label stands for; null when the repository no longer has it. A joined
    `origin=topic` label stands for the local branch, which its menu then acts on. */
export function labelTarget(
  label: { text: string; kind: "head" | "local" | "remote" | "tag" | "stash"; name?: string },
  branches: readonly Branch[],
  tags: readonly Tag[],
): { ref: RefTarget; branch: Branch | null; tag: Tag | null } | null {
  if (label.kind === "tag") {
    const tag = tags.find((entry) => entry.name === label.text);
    return tag ? { ref: { kind: "tag", name: tag.name, isHead: false }, branch: null, tag } : null;
  }
  if (label.kind === "stash") return null;
  const wanted = label.kind === "remote" ? "remote" : "local";
  const name = label.name ?? label.text;
  const branch = branches.find((entry) => entry.kind === wanted && entry.name === name);
  if (!branch) return null;
  return {
    ref: { kind: wanted === "remote" ? "remote" : "branch", name: branch.name, isHead: branch.isHead },
    branch,
    tag: null,
  };
}

/** A tag row keeps its full ref in `rev`: the label may be only the part after a folder. */
export function tagNameOf(node: { label: string; rev?: string }): string {
  return node.rev?.startsWith("refs/tags/") ? node.rev.slice("refs/tags/".length) : node.label;
}

/** The local branch `git switch` makes for a remote one: `origin/topic` becomes `topic`. */
export function localNameOf(remoteBranch: string, remotes: readonly string[]): string {
  const owner = [...remotes]
    .sort((a, b) => b.length - a.length)
    .find((remote) => remoteBranch.startsWith(`${remote}/`));
  return owner ? remoteBranch.slice(owner.length + 1) : remoteBranch.slice(remoteBranch.indexOf("/") + 1);
}
