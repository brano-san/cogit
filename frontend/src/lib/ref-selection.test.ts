import { describe, expect, it } from "vitest";
import { buildRefTree, type RefNode, type RefTreeInput } from "./ref-nodes";
import {
  branchSelectionMenu,
  commitSelectionMenu,
  refPicks,
  replayOrder,
  selectedNodes,
  selectionDeletable,
  selectionDeletion,
  selectionQuestion,
  toggleSelected,
} from "./ref-selection";

function node(id: string, kind: RefNode["kind"], extra: Partial<RefNode> = {}): RefNode {
  return { id, kind, label: id.split(":")[1] ?? id, depth: 1, ...extra } as RefNode;
}
const local = (name: string, extra: Partial<RefNode> = {}) =>
  node(`local:${name}`, "local", { branch: { name, kind: "local" } as never, rev: name, ...extra });
const remote = (name: string) =>
  node(`remote:${name}`, "remote", { branch: { name, kind: "remote" } as never, label: name.split("/")[1] as string });
const tag = (name: string) => node(`tag:${name}`, "tag", { tag: { name } as never });

describe("selectionDeletion", () => {
  it("leaves out the current branch and a worktree's, with the reason", () => {
    const plans = selectionDeletion(
      [local("main", { current: true }), local("wt", { worktree: { path: "x" } as never }), local("topic")],
      ["origin"],
    );
    expect(plans).toHaveLength(1);
    expect(plans[0]?.names).toEqual(["topic"]);
    expect(plans[0]?.skipped.map((each) => each.name)).toEqual(["main", "wt"]);
  });

  it("groups remote branches by remote and keeps tags apart", () => {
    const plans = selectionDeletion(
      [remote("origin/a"), remote("up/b"), remote("origin/c"), tag("v1")],
      ["origin", "up"],
    );
    expect(plans.map((plan) => [plan.kind, plan.remote, plan.names])).toEqual([
      ["remoteBranch", "origin", ["origin/a", "origin/c"]],
      ["remoteBranch", "up", ["up/b"]],
      ["tag", null, ["v1"]],
    ]);
  });

  it("ignores headings, folders and origin/HEAD", () => {
    const head = { ...remote("origin/HEAD"), label: "HEAD" };
    expect(selectionDeletion([node("group:local", "group"), node("folder:f", "folder"), head], ["origin"])).toEqual([]);
  });

  it("is not deletable when everything was left out", () => {
    expect(selectionDeletable(selectionDeletion([local("main", { current: true })], []))).toBe(false);
    expect(selectionDeletable(selectionDeletion([local("a")], []))).toBe(true);
  });
});

describe("selectionQuestion", () => {
  it("counts each kind, warns apart about remotes and explains what is left out", () => {
    const text = selectionQuestion(
      selectionDeletion(
        [local("main", { current: true }), local("a"), remote("origin/b"), remote("origin/c")],
        ["origin"],
      ),
    );
    expect(text).toContain("Delete 1 branch, 2 remote branches?");
    expect(text).toContain("Undo can bring back");
    expect(text).toContain("git push --delete");
    expect(text).toContain("Not included (1): main, it is the current branch.");
  });

  it("says nothing of servers when no remote branch goes", () => {
    const text = selectionQuestion(selectionDeletion([tag("v1"), tag("v2")], []));
    expect(text).toContain("Delete 2 tags?");
    expect(text).not.toContain("push --delete");
  });
});

describe("branchSelectionMenu", () => {
  const facts = (over = {}) => ({
    picks: refPicks([local("a"), local("b")]),
    hasRemote: true,
    deletable: true,
    toggle: null,
    ...over,
  });
  const byLabel = (items: ReturnType<typeof branchSelectionMenu>) =>
    new Map(items.filter((entry) => !entry.separator).map((entry) => [entry.label, entry]));

  it("enables only Push To, Delete, Copy and Toggle", () => {
    const on = branchSelectionMenu(facts())
      .filter((entry) => !entry.separator && entry.enabled)
      .map((entry) => entry.label);
    expect(on).toEqual(["Push To…", "Delete…", "Copy", "Toggle"]);
  });

  it("keeps the other items visible, off, with a reason", () => {
    const items = byLabel(branchSelectionMenu(facts()));
    expect(items.get("Merge (several selected)")?.enabled).toBe(false);
    expect(items.get("Check Out (several selected)")?.enabled).toBe(false);
  });

  it("gives Toggle its key", () => {
    expect(byLabel(branchSelectionMenu(facts())).get("Toggle")?.accelerator).toBe("Space");
  });

  it("switches Push To off without a local branch or a remote", () => {
    const tagsOnly = branchSelectionMenu(facts({ picks: refPicks([tag("v1")]) }));
    expect(byLabel(tagsOnly).get("Push To… (no local branch selected)")?.enabled).toBe(false);
    const none = branchSelectionMenu(facts({ hasRemote: false }));
    expect(byLabel(none).get("Push To… (no remote)")?.enabled).toBe(false);
  });

  it("switches Delete off when nothing can go", () => {
    const items = byLabel(branchSelectionMenu(facts({ deletable: false })));
    expect(items.get("Delete… (nothing here can be deleted)")?.enabled).toBe(false);
  });
});

describe("commitSelectionMenu", () => {
  it("enables only the four that act on a group", () => {
    const items = commitSelectionMenu().filter((entry) => !entry.separator);
    expect(items.filter((entry) => entry.enabled).map((entry) => entry.label)).toEqual([
      "Cherry-Pick…",
      "Revert…",
      "Copy Message",
      "Copy ID",
    ]);
    expect(items.length).toBeGreaterThan(10);
  });

  it("has no doubled or edge separators", () => {
    const items = commitSelectionMenu();
    expect(items[0]?.separator).toBe(false);
    expect(items.at(-1)?.separator).toBe(false);
    expect(items.some((entry, at) => entry.separator && items[at - 1]?.separator)).toBe(false);
  });
});

describe("replayOrder", () => {
  const rows = new Map([
    ["new", 1],
    ["mid", 5],
    ["old", 9],
  ]);

  it("puts the oldest first for a cherry-pick, whatever was clicked first", () => {
    expect(replayOrder(rows, ["mid", "new", "old"], "oldest-first")).toEqual(["old", "mid", "new"]);
  });

  it("puts the newest first for a revert", () => {
    expect(replayOrder(rows, ["old", "new", "mid"], "newest-first")).toEqual(["new", "mid", "old"]);
  });
});

describe("toggleSelected and selectedNodes", () => {
  const input = {
    head: { kind: "branch", name: "main", oid: "1" },
    branches: [
      { name: "main", kind: "local", isHead: true, oid: "1", upstream: null, ahead: 0, behind: 0 },
      { name: "a", kind: "local", isHead: false, oid: "2", upstream: null, ahead: 0, behind: 0 },
      { name: "b", kind: "local", isHead: false, oid: "3", upstream: null, ahead: 0, behind: 0 },
    ],
    tags: [],
    stashes: [],
    lost: [],
    remoteUrls: {},
    collapsed: new Set<string>(),
    filter: "",
  } as unknown as RefTreeInput;
  const tree = buildRefTree(input);

  it("turns every box on, then all off when all are on", () => {
    const first = toggleSelected(tree, ["local:a", "local:b"], new Set(["local:a"]));
    expect([...first].sort()).toEqual(["local:a", "local:b"]);
    expect([...toggleSelected(tree, ["local:a", "local:b"], first)]).toEqual([]);
  });

  it("changes nothing when no selected row can be ticked", () => {
    expect([...toggleSelected(tree, ["nope"], new Set(["local:a"]))]).toEqual(["local:a"]);
  });

  it("lists selected rows in tree order", () => {
    const rows = selectedNodes(tree, new Set(["local:b", "local:a"]));
    expect(rows.map((row) => row.id)).toEqual(["local:a", "local:b"]);
  });
});
