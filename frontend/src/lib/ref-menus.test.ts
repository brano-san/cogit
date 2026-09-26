import { describe, expect, it } from "vitest";
import type { Branch, ContextItem, Tag } from "./ipc";
import {
  branchesBranchMenu,
  branchesStashMenu,
  branchesTagMenu,
  commitFacts,
  graphCommitMenu,
  graphRefMenu,
  labelTarget,
  localNameOf,
  tagNameOf,
  workingTreeMenu,
  type BranchesContext,
  type CommitFacts,
  type RefTarget,
} from "./ref-menus";

/** The menu as a reader sees it: labels without reasons, `—` for a separator. */
function shape(menu: ContextItem[]): string[] {
  return menu.map((entry) => (entry.separator ? "—" : entry.label.replace(/ \(.*\)$/, "")));
}

function find(menu: ContextItem[], label: string): ContextItem {
  const found = menu.find((entry) => !entry.separator && entry.label.replace(/ \(.*\)$/, "") === label);
  if (!found) throw new Error(`no ${label} in ${shape(menu).join(", ")}`);
  return found;
}

function wellFormed(menu: ContextItem[]) {
  expect(menu.at(0)?.separator).toBe(false);
  expect(menu.at(-1)?.separator).toBe(false);
  expect(menu.some((entry, at) => entry.separator && menu[at + 1]?.separator)).toBe(false);
  expect(new Set(menu.filter((e) => !e.separator).map((e) => e.id)).size).toBe(
    menu.filter((e) => !e.separator).length,
  );
}

const older: CommitFacts = {
  isHeadCommit: false,
  detachedHere: false,
  onHead: true,
  published: false,
  parents: 1,
  upstream: "origin/main",
  hasRemote: true,
};
const pushed: CommitFacts = { ...older, published: true };
const elsewhere: CommitFacts = { ...older, onHead: false };
const branch: RefTarget = { kind: "branch", name: "topic", isHead: false };
const rows: BranchesContext = { selected: "sel", oid: "abc", untickable: null };

describe("graphCommitMenu (#38)", () => {
  it("lists the items in the order and groups of the task", () => {
    const menu = graphCommitMenu(older);
    expect(shape(menu)).toEqual([
      "Check Out",
      "Merge",
      "Cherry-Pick",
      "Revert",
      "Rebase",
      "—",
      "Modify",
      "Split",
      "Squash",
      "Edit Message",
      "Edit Author",
      "Rebase Interactive From",
      "—",
      "Add Branch",
      "Add Tag",
      "—",
      "Reset",
      "Reset Advanced…",
      "Roll Back Tree",
      "—",
      "Push Up To",
      "—",
      "Copy Message",
      "Copy ID",
    ]);
    wellFormed(menu);
  });

  // The Diff panel's commit buttons are gone: every one of them has to be here instead.
  it("carries every action the commit details used to offer as buttons", () => {
    for (const facts of [older, pushed, { ...older, isHeadCommit: true }]) {
      const menu = graphCommitMenu(facts);
      for (const label of ["Cherry-Pick", "Revert", "Split", "Rebase Interactive From", "Roll Back Tree"]) {
        find(menu, label);
      }
    }
    expect(find(graphCommitMenu(older), "Roll Back Tree")).toMatchObject({
      id: "ref:rollback",
      enabled: true,
    });
    expect(find(graphRefMenu(branch, older), "Roll Back Tree").enabled).toBe(true);
  });

  it("turns Squash off on a pushed commit and says why", () => {
    const squash = find(graphCommitMenu(pushed), "Squash");
    expect(squash.enabled).toBe(false);
    expect(squash.label).toBe("Squash (already pushed)");
  });

  it("keeps the other rewrites on for a pushed commit: they warn when run", () => {
    const menu = graphCommitMenu(pushed);
    for (const label of ["Modify", "Split", "Edit Message", "Edit Author"]) {
      expect(find(menu, label).enabled).toBe(true);
    }
  });

  it("cannot rewrite a commit that is not in HEAD's history", () => {
    const menu = graphCommitMenu(elsewhere);
    for (const label of ["Modify", "Split", "Squash", "Edit Message", "Edit Author", "Rebase Interactive From"]) {
      expect(find(menu, label).enabled).toBe(false);
    }
    expect(find(menu, "Merge").enabled).toBe(true);
    expect(find(menu, "Cherry-Pick").enabled).toBe(true);
  });

  it("does not merge, cherry-pick or rebase onto what HEAD already has", () => {
    const menu = graphCommitMenu(older);
    for (const label of ["Merge", "Cherry-Pick", "Rebase"]) {
      expect(find(menu, label).enabled).toBe(false);
    }
    expect(find(menu, "Revert").enabled).toBe(true);
  });

  it("refuses to squash the first commit or rewrite a merge", () => {
    expect(find(graphCommitMenu({ ...older, parents: 0 }), "Squash").enabled).toBe(false);
    const merge = graphCommitMenu({ ...older, parents: 2 });
    expect(find(merge, "Modify").label).toMatch(/merge commit/);
    expect(find(merge, "Revert").enabled).toBe(false);
  });

  it("has nothing to rebase interactively after the HEAD commit, nor to reset to", () => {
    const menu = graphCommitMenu({ ...older, isHeadCommit: true });
    expect(find(menu, "Rebase Interactive From").enabled).toBe(false);
    expect(find(menu, "Reset").enabled).toBe(false);
    expect(find(menu, "Reset Advanced…").enabled).toBe(true);
  });

  it("offers Push Up To only for an unpushed commit of a branch with an upstream", () => {
    expect(find(graphCommitMenu(older), "Push Up To").enabled).toBe(true);
    expect(find(graphCommitMenu(pushed), "Push Up To").enabled).toBe(false);
    expect(find(graphCommitMenu({ ...older, upstream: null }), "Push Up To").label).toMatch(/upstream/);
    expect(find(graphCommitMenu({ ...older, hasRemote: false }), "Push Up To").label).toMatch(/no remote/);
  });

  it("does not check out the commit HEAD is already detached at", () => {
    expect(find(graphCommitMenu({ ...older, detachedHere: true }), "Check Out").enabled).toBe(false);
    expect(find(graphCommitMenu(older), "Check Out").enabled).toBe(true);
  });
});

describe("graphRefMenu (#39)", () => {
  it("lists the items in the order and groups of the task", () => {
    const menu = graphRefMenu(branch, elsewhere);
    expect(shape(menu)).toEqual([
      "Check Out",
      "Merge",
      "Cherry-Pick",
      "Revert",
      "Rebase",
      "—",
      "Modify",
      "Split",
      "Squash",
      "Edit Message",
      "Edit Author",
      "Rebase Interactive From",
      "—",
      "Add Branch",
      "Add Tag",
      "—",
      "Reset",
      "Reset Advanced…",
      "Roll Back Tree",
      "—",
      "Push",
      "Push To…",
      "Set Upstream…",
      "Stop Tracking",
      "—",
      "Delete",
      "Rename",
      "—",
      "Copy Name",
      "Copy Message",
      "Copy ID",
    ]);
    wellFormed(menu);
  });

  it("disables Rename of a pushed branch or tag with the reason", () => {
    for (const ref of [branch, { kind: "tag", name: "v1", isHead: false } as const]) {
      const rename = find(graphRefMenu(ref, pushed), "Rename");
      expect(rename.enabled).toBe(false);
      expect(rename.label).toMatch(/already pushed/);
    }
    expect(find(graphRefMenu(branch, older), "Rename").enabled).toBe(true);
  });

  it("does not check out, merge, rebase onto or delete the current branch", () => {
    const menu = graphRefMenu({ ...branch, isHead: true }, { ...older, isHeadCommit: true });
    for (const label of ["Check Out", "Merge", "Rebase", "Delete"]) {
      expect(find(menu, label).enabled).toBe(false);
    }
  });

  it("does not push or rename a remote branch", () => {
    const menu = graphRefMenu({ kind: "remote", name: "origin/topic", isHead: false }, pushed);
    for (const label of ["Push", "Push To…", "Rename"]) {
      expect(find(menu, label).label).toMatch(/remote branch/);
    }
    expect(find(menu, "Delete").enabled).toBe(true);
  });
});

describe("branchesBranchMenu (#33)", () => {
  it("lists the items in the order and groups of the task", () => {
    const menu = branchesBranchMenu(branch, elsewhere, rows);
    expect(shape(menu)).toEqual([
      "Check Out",
      "—",
      "Reveal Commit",
      "Compare with HEAD",
      "Compare with Selected Commit",
      "—",
      "Merge",
      "Rebase",
      "—",
      "Push",
      "Push To…",
      "Set Upstream…",
      "Stop Tracking",
      "—",
      "Reset",
      "Reset Advanced…",
      "—",
      "Delete",
      "—",
      "Copy",
      "—",
      "Toggle",
    ]);
    wellFormed(menu);
  });

  it("cannot compare with a selection that is missing or is the same commit", () => {
    expect(
      find(branchesBranchMenu(branch, elsewhere, { ...rows, selected: null }), "Compare with Selected Commit")
        .enabled,
    ).toBe(false);
    expect(
      find(branchesBranchMenu(branch, elsewhere, { ...rows, selected: "abc" }), "Compare with Selected Commit")
        .enabled,
    ).toBe(false);
    expect(find(branchesBranchMenu(branch, elsewhere, rows), "Compare with Selected Commit").enabled).toBe(
      true,
    );
  });

  it("has nothing to compare with HEAD on HEAD's own commit", () => {
    const menu = branchesBranchMenu(branch, { ...older, isHeadCommit: true }, rows);
    expect(find(menu, "Compare with HEAD").enabled).toBe(false);
  });

  it("toggles only what can be ticked", () => {
    const menu = branchesBranchMenu(branch, elsewhere, { ...rows, untickable: "no commit" });
    expect(find(menu, "Toggle").enabled).toBe(false);
  });
});

describe("upstream rows (F-130)", () => {
  const tracking: RefTarget = { ...branch, upstream: "origin/topic" };
  const menus = (ref: RefTarget, facts: CommitFacts) => [
    graphRefMenu(ref, facts),
    branchesBranchMenu(ref, facts, rows),
  ];

  it("sets and stops the upstream of a local branch that tracks one", () => {
    for (const menu of menus(tracking, older)) {
      expect(find(menu, "Set Upstream…").enabled).toBe(true);
      expect(find(menu, "Stop Tracking").enabled).toBe(true);
    }
  });

  it("has nothing to stop tracking on a branch without an upstream", () => {
    for (const menu of menus({ ...branch, upstream: null }, older)) {
      expect(find(menu, "Set Upstream…").enabled).toBe(true);
      const stop = find(menu, "Stop Tracking");
      expect(stop.enabled).toBe(false);
      expect(stop.label).toMatch(/no upstream/);
    }
  });

  it("has no remote branch to track without a remote", () => {
    for (const menu of menus(tracking, { ...older, hasRemote: false })) {
      expect(find(menu, "Set Upstream…").label).toMatch(/no remote/);
    }
  });

  it("gives a remote branch or a tag no upstream to set or stop", () => {
    const remote: RefTarget = { kind: "remote", name: "origin/topic", isHead: false };
    for (const menu of menus(remote, older)) {
      expect(find(menu, "Set Upstream…").label).toMatch(/a remote branch/);
      expect(find(menu, "Stop Tracking").label).toMatch(/a remote branch/);
    }
    const tag = graphRefMenu({ kind: "tag", name: "v1", isHead: false }, older);
    expect(find(tag, "Set Upstream…").label).toMatch(/a tag/);
    expect(find(tag, "Stop Tracking").label).toMatch(/a tag/);
  });
});

describe("branchesTagMenu (#34)", () => {
  it("lists the items in the order and groups of the task", () => {
    const menu = branchesTagMenu(elsewhere, { ...rows, annotated: true });
    expect(shape(menu)).toEqual([
      "Check Out",
      "—",
      "Reveal Commit",
      "Compare with HEAD",
      "Compare with Selected Commit",
      "—",
      "Merge",
      "—",
      "Push",
      "Push To…",
      "—",
      "Reset",
      "Reset Advanced…",
      "—",
      "Delete",
      "—",
      "Copy",
      "Copy Message",
      "—",
      "Toggle",
    ]);
    wellFormed(menu);
  });

  it("has no annotation to copy on a lightweight tag", () => {
    const menu = branchesTagMenu(elsewhere, { ...rows, annotated: false });
    expect(find(menu, "Copy Message").enabled).toBe(false);
    expect(find(menu, "Copy Message").label).toMatch(/lightweight/);
  });

  it("turns off what needs a commit on a tag of a tree", () => {
    const menu = branchesTagMenu(elsewhere, { ...rows, oid: null, annotated: true, untickable: "not a commit" });
    for (const label of ["Check Out", "Reveal Commit", "Compare with HEAD", "Merge", "Reset", "Toggle"]) {
      expect(find(menu, label).enabled).toBe(false);
    }
    expect(find(menu, "Delete").enabled).toBe(true);
  });
});

describe("branchesStashMenu (#35)", () => {
  it("lists the items in the order and groups of the task", () => {
    const menu = branchesStashMenu(elsewhere, rows);
    expect(shape(menu)).toEqual([
      "Apply Stash",
      "Pop Stash",
      "—",
      "Reveal Commit",
      "Compare with HEAD",
      "Compare with Selected Commit",
      "—",
      "Rename Stash",
      "—",
      "Drop Stash",
      "—",
      "Copy Message",
      "—",
      "Toggle",
    ]);
    wellFormed(menu);
  });
});

describe("branchesStashMenu Pop (F-040)", () => {
  it("offers Pop beside Apply: apply and drop the entry in one go", () => {
    const pop = find(branchesStashMenu(elsewhere, rows), "Pop Stash");
    expect(pop.enabled).toBe(true);
  });
});

describe("workingTreeMenu (#37)", () => {
  it("offers Commit, Stage, Unstage and Discard in that order", () => {
    const menu = workingTreeMenu({ staged: 1, modified: 1, untracked: 1 });
    expect(shape(menu)).toEqual(["Commit", "Stage", "Unstage", "Discard"]);
    expect(menu.every((entry) => entry.enabled)).toBe(true);
    wellFormed(menu);
  });

  it("keeps every row but turns off what a clean tree cannot do", () => {
    const menu = workingTreeMenu({ staged: 0, modified: 0, untracked: 0 });
    expect(shape(menu)).toEqual(["Commit", "Stage", "Unstage", "Discard"]);
    expect(menu.some((entry) => entry.enabled)).toBe(false);
  });

  it("stages untracked files but does not offer to discard them", () => {
    const menu = workingTreeMenu({ staged: 0, modified: 0, untracked: 2 });
    expect(find(menu, "Stage").enabled).toBe(true);
    expect(find(menu, "Discard").enabled).toBe(false);
    expect(find(menu, "Unstage").enabled).toBe(false);
  });
});

describe("commitFacts", () => {
  const main: Branch = {
    name: "main",
    fullName: "refs/heads/main",
    kind: "local",
    oid: "h",
    isHead: true,
    upstream: "origin/main",
    ahead: 0,
    behind: 0,
  };
  const base = { parents: 1, onHead: true, published: false, hasRemote: true, branches: [main] };

  it("knows HEAD's own commit and the upstream Push Up To sends to", () => {
    const facts = commitFacts({ ...base, oid: "h", head: { kind: "branch", name: "main", oid: "h" } });
    expect(facts.isHeadCommit).toBe(true);
    expect(facts.detachedHere).toBe(false);
    expect(facts.upstream).toBe("origin/main");
  });

  it("has no upstream while HEAD is detached", () => {
    const facts = commitFacts({ ...base, oid: "h", head: { kind: "detached", oid: "h" } });
    expect(facts.detachedHere).toBe(true);
    expect(facts.upstream).toBeNull();
  });

  it("has no HEAD commit in an unborn repository", () => {
    const facts = commitFacts({ ...base, oid: "x", head: { kind: "unborn", name: "main" } });
    expect(facts.isHeadCommit).toBe(false);
  });
});

describe("labelTarget", () => {
  const branches: Branch[] = [
    { name: "topic", fullName: "refs/heads/topic", kind: "local", oid: "a", isHead: true, upstream: null, ahead: 0, behind: 0 },
    { name: "origin/topic", fullName: "refs/remotes/origin/topic", kind: "remote", oid: "a", isHead: false, upstream: null, ahead: 0, behind: 0 },
  ];
  const tags: Tag[] = [{ name: "v1", fullName: "refs/tags/v1", oid: "a", isAnnotated: false, pointsToCommit: true }];

  it("finds the branch, remote branch or tag a label stands for", () => {
    expect(labelTarget({ text: "topic", kind: "head" }, branches, tags)?.ref).toEqual({
      kind: "branch",
      name: "topic",
      isHead: true,
      upstream: null,
    });
    expect(labelTarget({ text: "origin/topic", kind: "remote" }, branches, tags)?.ref.kind).toBe("remote");
    expect(labelTarget({ text: "v1", kind: "tag" }, branches, tags)?.tag?.name).toBe("v1");
  });

  it("acts on the local branch of a joined origin=branch label", () => {
    const joined = { text: "origin=topic", kind: "head", remotes: ["origin"], name: "topic" } as const;
    expect(labelTarget(joined, branches, tags)?.branch?.kind).toBe("local");
  });

  it("carries a local branch's upstream for Stop Tracking", () => {
    const tracked = branches.map((entry) =>
      entry.kind === "local" ? { ...entry, upstream: "origin/topic" } : entry,
    );
    expect(labelTarget({ text: "topic", kind: "local" }, tracked, tags)?.ref.upstream).toBe("origin/topic");
  });

  it("leaves a stash label to the stash menu", () => {
    expect(labelTarget({ text: "stash@{0}", kind: "stash" }, branches, tags)).toBeNull();
  });

  it("gives nothing for a ref that is gone", () => {
    expect(labelTarget({ text: "gone", kind: "local" }, branches, tags)).toBeNull();
  });
});

describe("names", () => {
  it("reads a tag's full name from its ref, not from a folder-shortened label", () => {
    expect(tagNameOf({ label: "1.0", rev: "refs/tags/release/1.0" })).toBe("release/1.0");
    expect(tagNameOf({ label: "v1" })).toBe("v1");
  });

  it("names the local branch a remote one checks out as", () => {
    expect(localNameOf("origin/feature/x", ["origin"])).toBe("feature/x");
    expect(localNameOf("team/mirror/main", ["origin", "team/mirror"])).toBe("main");
  });
});
