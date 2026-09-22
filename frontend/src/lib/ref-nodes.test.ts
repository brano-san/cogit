import { describe, expect, it } from "vitest";
import type { Branch, CommitRow, Head, StashEntry, Tag } from "$lib/ipc";
import {
  buildRefTree,
  checkState,
  defaultVisible,
  leavesUnder,
  toggleNode,
  visibleTips,
  type RefNode,
  type RefTreeInput,
} from "./ref-nodes";
import { flatten } from "./tree";

const OID = "a".repeat(40);

function branch(name: string, over: Partial<Branch> = {}): Branch {
  return {
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid: OID,
    isHead: false,
    upstream: null,
    ahead: 0,
    behind: 0,
    ...over,
  };
}

function tag(name: string, over: Partial<Tag> = {}): Tag {
  return {
    name,
    fullName: `refs/tags/${name}`,
    oid: OID,
    isAnnotated: false,
    pointsToCommit: true,
    ...over,
  };
}

function stash(index: number): StashEntry {
  return { index, oid: OID, message: `WIP ${index}`, timestamp: 0 };
}

function lost(oid: string): CommitRow {
  return {
    oid,
    parents: [],
    summary: "orphan",
    authorName: "a",
    authorEmail: "a@b",
    timestamp: 0,
    tzOffsetMinutes: 0,
  };
}

const head: Head = { kind: "branch", name: "master", oid: OID };

function input(over: Partial<RefTreeInput> = {}): RefTreeInput {
  return {
    head,
    branches: [],
    tags: [],
    stashes: [],
    lost: [],
    remoteUrls: {},
    collapsed: new Set(),
    filter: "",
    ...over,
  };
}

const ids = (nodes: RefNode[]) => nodes.map((node) => node.id);

describe("buildRefTree", () => {
  it("puts HEAD on the first row with its short oid", () => {
    const [first] = buildRefTree(input());
    expect(first?.id).toBe("HEAD");
    expect(first?.label).toContain(OID.slice(0, 7));
  });

  it("groups local branches under one heading with a count", () => {
    const nodes = buildRefTree(input({ branches: [branch("master"), branch("topic")] }));
    const group = nodes.find((node) => node.id === "group:local");
    expect(group?.label).toBe("Local Branches (2)");
  });

  it("marks the checked-out branch with a filled triangle", () => {
    const nodes = buildRefTree(input({ branches: [branch("master", { isHead: true })] }));
    expect(nodes.find((node) => node.id === "local:master")?.marker).toBe("▶");
  });

  it("marks a remote branch with a hollow triangle", () => {
    const remote = branch("origin/master", { kind: "remote", fullName: "refs/remotes/origin/master" });
    const nodes = buildRefTree(input({ branches: [remote] }));
    expect(nodes.find((node) => node.id === "remote:origin/master")?.marker).toBe("▷");
  });

  it("says a branch is level with its upstream", () => {
    const nodes = buildRefTree(input({ branches: [branch("master", { upstream: "origin/master" })] }));
    expect(nodes.find((node) => node.id === "local:master")?.detail).toBe("= origin");
  });

  it("counts divergence instead when there is any", () => {
    const diverged = branch("master", { upstream: "origin/master", ahead: 2, behind: 1 });
    const nodes = buildRefTree(input({ branches: [diverged] }));
    expect(nodes.find((node) => node.id === "local:master")?.detail).toBe("↑2 ↓1");
  });

  it("shows the remote url beside its name", () => {
    const remote = branch("origin/master", { kind: "remote", fullName: "refs/remotes/origin/master" });
    const nodes = buildRefTree(
      input({ branches: [remote], remoteUrls: { origin: "git@github.com:x/y.git" } }),
    );
    const group = nodes.find((node) => node.id === "remote-group:origin");
    expect(group?.label).toBe("origin (1)");
    expect(group?.detail).toBe("git@github.com:x/y.git");
  });

  it("gives tags, stashes and lost commits their own groups", () => {
    const nodes = buildRefTree(
      input({ tags: [tag("v1")], stashes: [stash(0)], lost: [lost("b".repeat(40))] }),
    );
    expect(ids(nodes)).toContain("group:tags");
    expect(ids(nodes)).toContain("group:stashes");
    expect(ids(nodes)).toContain("group:lost");
  });

  it("leaves out a group that has nothing in it", () => {
    expect(ids(buildRefTree(input()))).not.toContain("group:tags");
  });

  it("builds the whole tree; hiding is the folding step's job", () => {
    const collapsed = new Set(["group:local"]);
    const nodes = buildRefTree(input({ branches: [branch("master")], collapsed }));
    expect(ids(nodes)).toContain("group:local");
    expect(ids(nodes)).toContain("local:master");
  });

  // The reported case: folding Tags took its ticks with it, because the rows were not
  // built at all and the heading counted nothing under it (R-158).
  it("builds the rows of a folded Tags, Stashes and Lost group too", () => {
    const collapsed = new Set(["group:tags", "group:stashes", "group:lost"]);
    const nodes = buildRefTree(
      input({ tags: [tag("v1")], stashes: [stash(0)], lost: [lost("c".repeat(40))], collapsed }),
    );
    expect(ids(nodes)).toEqual(
      expect.arrayContaining(["tag:v1", "stash:0", `lost:${"c".repeat(40)}`]),
    );
  });

  it("hides the children of a collapsed group but keeps the group", () => {
    const collapsed = new Set(["group:local"]);
    const rows = flatten(buildRefTree(input({ branches: [branch("master")] })), collapsed);
    expect(ids(rows)).toContain("group:local");
    expect(ids(rows)).not.toContain("local:master");
  });

  /** The reported symptom: a folder's caret changed its glyph and nothing else. */
  it("folds a folder inside a group, not only the group", () => {
    const nodes = buildRefTree(input({ branches: [branch("feature/auth/login")] }));
    const folder = nodes.find((node) => node.kind === "folder");
    const rows = flatten(nodes, new Set([folder?.id ?? ""]));

    expect(ids(rows)).toContain(folder?.id);
    expect(ids(rows)).not.toContain("local:feature/auth/login");
    expect(ids(rows)).toContain("group:local");
  });

  it("splits a slashed branch name into folders", () => {
    const nodes = buildRefTree(input({ branches: [branch("feature/auth/login")] }));
    const labels = nodes.filter((node) => node.kind === "folder").map((node) => node.label);
    expect(labels).toEqual(["feature", "auth"]);
  });

  it("keeps a branch whose name matches the filter and drops the rest", () => {
    const nodes = buildRefTree(
      input({ branches: [branch("master"), branch("topic")], filter: "top" }),
    );
    expect(ids(nodes)).toContain("local:topic");
    expect(ids(nodes)).not.toContain("local:master");
  });

  it("matches a filter against the oid too", () => {
    const nodes = buildRefTree(input({ branches: [branch("master")], filter: OID.slice(0, 6) }));
    expect(ids(nodes)).toContain("local:master");
  });
});

describe("leavesUnder", () => {
  it("does not let a folder claim the branches of the folder next to it", () => {
    const nodes = buildRefTree(input({ branches: [branch("fix/a"), branch("feat/b")] }));
    expect(leavesUnder(nodes, "folder:local/fix")).toEqual(["local:fix/a"]);
    expect(leavesUnder(nodes, "folder:local/feat")).toEqual(["local:feat/b"]);
  });

  it("leaves out a tag that points at no commit", () => {
    const nodes = buildRefTree(
      input({ tags: [tag("v1"), tag("v-tree", { pointsToCommit: false })] }),
    );
    expect(leavesUnder(nodes, "group:tags")).toEqual(["tag:v1"]);
  });

  it("lists every tickable row inside a group", () => {
    const nodes = buildRefTree(input({ branches: [branch("a"), branch("b")] }));
    expect(leavesUnder(nodes, "group:local")).toEqual(["local:a", "local:b"]);
  });

  it("treats a leaf as containing itself", () => {
    const nodes = buildRefTree(input({ branches: [branch("a")] }));
    expect(leavesUnder(nodes, "local:a")).toEqual(["local:a"]);
  });
});

describe("checkState", () => {
  const nodes = buildRefTree(input({ branches: [branch("a"), branch("b")] }));

  it("is off when nothing under it is ticked", () => {
    expect(checkState(nodes, "group:local", new Set())).toBe("off");
  });

  it("is on when everything under it is ticked", () => {
    expect(checkState(nodes, "group:local", new Set(["local:a", "local:b"]))).toBe("on");
  });

  it("is mixed when only some are", () => {
    expect(checkState(nodes, "group:local", new Set(["local:a"]))).toBe("mixed");
  });
});

describe("toggleNode", () => {
  const nodes = buildRefTree(input({ branches: [branch("a"), branch("b")] }));

  it("ticks a single row", () => {
    expect([...toggleNode(nodes, "local:a", new Set())]).toEqual(["local:a"]);
  });

  it("unticks a row that was ticked", () => {
    expect(toggleNode(nodes, "local:a", new Set(["local:a"])).size).toBe(0);
  });

  it("ticks the whole group when the group was empty", () => {
    const next = toggleNode(nodes, "group:local", new Set());
    expect([...next].sort()).toEqual(["local:a", "local:b"]);
  });

  // The standard three-state box: empty → all, mixed → all, all → none. The heading is
  // never stored; each step is read back from the children (R-158).
  it("fills a mixed group, and empties a full one", () => {
    let ticked: ReadonlySet<string> = new Set(["local:a"]);
    expect(checkState(nodes, "group:local", ticked)).toBe("mixed");

    ticked = toggleNode(nodes, "group:local", ticked);
    expect([...ticked].sort()).toEqual(["local:a", "local:b"]);
    expect(checkState(nodes, "group:local", ticked)).toBe("on");

    ticked = toggleNode(nodes, "group:local", ticked);
    expect(ticked.size).toBe(0);
    expect(checkState(nodes, "group:local", ticked)).toBe("off");

    ticked = toggleNode(nodes, "group:local", ticked);
    expect(checkState(nodes, "group:local", ticked)).toBe("on");
  });

  it("reads a folded group's state from its children, not from the rows on screen", () => {
    const folded = buildRefTree(
      input({ tags: [tag("v1"), tag("v2")], collapsed: new Set(["group:tags"]) }),
    );
    expect(checkState(folded, "group:tags", new Set(["tag:v1"]))).toBe("mixed");
  });

  it("makes a group mixed when one branch in a nested folder is ticked", () => {
    const nested = buildRefTree(input({ branches: [branch("fix/a"), branch("main")] }));
    const ticked = toggleNode(nested, "local:fix/a", new Set());
    expect(checkState(nested, "folder:local/fix", ticked)).toBe("on");
    expect(checkState(nested, "group:local", ticked)).toBe("mixed");
  });

  it("never ticks a tag that points at no commit, alone or with its group", () => {
    const withTree = buildRefTree(
      input({ tags: [tag("v1"), tag("v-tree", { pointsToCommit: false })] }),
    );
    expect(toggleNode(withTree, "tag:v-tree", new Set()).size).toBe(0);
    const all = toggleNode(withTree, "group:tags", new Set());
    expect([...all]).toEqual(["tag:v1"]);
    expect(checkState(withTree, "group:tags", all)).toBe("on");
  });

  it("leaves rows outside the group alone", () => {
    const withTag = buildRefTree(input({ branches: [branch("a")], tags: [tag("v1")] }));
    const next = toggleNode(withTag, "group:local", new Set(["tag:v1"]));
    expect(next.has("tag:v1")).toBe(true);
  });
});

describe("visibleTips", () => {
  it("turns ticked ids into revisions the backend can resolve", () => {
    const nodes = buildRefTree(
      input({ branches: [branch("master")], tags: [tag("v1")], stashes: [stash(2)] }),
    );
    const ticked = new Set(["HEAD", "local:master", "tag:v1", "stash:2"]);
    expect(visibleTips(nodes, ticked).sort()).toEqual([
      "HEAD",
      "refs/heads/master",
      "refs/tags/v1",
      "stash@{2}",
    ]);
  });

  it("sends a lost commit as its raw oid", () => {
    const oid = "c".repeat(40);
    const nodes = buildRefTree(input({ lost: [lost(oid)] }));
    expect(visibleTips(nodes, new Set([`lost:${oid}`]))).toEqual([oid]);
  });

  it("never sends a tag that points at no commit", () => {
    const nodes = buildRefTree(input({ tags: [tag("v-tree", { pointsToCommit: false })] }));
    expect(visibleTips(nodes, new Set(["tag:v-tree"]))).toEqual([]);
  });

  it("is empty when nothing is ticked, which means show nothing", () => {
    const nodes = buildRefTree(input({ branches: [branch("master")] }));
    expect(visibleTips(nodes, new Set())).toEqual([]);
  });
});

describe("defaultVisible", () => {
  // The graph is the union of what the ticked refs reach; out of the box that is HEAD
  // alone. Labels of every other ref still show on the commits it reaches (R-158).
  it("starts with HEAD alone ticked", () => {
    const remote = branch("origin/master", { kind: "remote", fullName: "refs/remotes/origin/master" });
    const nodes = buildRefTree(input({ branches: [branch("master"), remote], tags: [tag("v1")] }));
    expect([...defaultVisible(nodes)]).toEqual(["HEAD"]);
  });

  it("ticks HEAD on a detached checkout too", () => {
    const nodes = buildRefTree(input({ head: { kind: "detached", oid: OID } }));
    expect([...defaultVisible(nodes)]).toEqual(["HEAD"]);
  });
});

describe("node ids are unique", () => {
  // `fix/14340` existing both locally and on origin gave two nodes called `folder:fix`.
  // A keyed `{#each}` with a duplicate key renders unpredictably: rows go missing, a
  // heading expands every other click, and collapsing one folder collapses another.
  it("gives two folders of the same name in different groups different ids", () => {
    const nodes = buildRefTree(
      input({
        branches: [
          branch("fix/14340"),
          branch("origin/fix/14340", { kind: "remote" }),
        ],
      }),
    );

    expect(new Set(ids(nodes)).size).toBe(ids(nodes).length);
  });

  it("keeps every id unique across locals, remotes and tags at once", () => {
    const nodes = buildRefTree(
      input({
        branches: [
          branch("feature/a"),
          branch("feature/b"),
          branch("origin/feature/a", { kind: "remote" }),
          branch("upstream/feature/a", { kind: "remote" }),
        ],
        tags: [tag("feature/a")],
      }),
    );

    expect(new Set(ids(nodes)).size).toBe(ids(nodes).length);
  });

  it("still folds the two folders independently", () => {
    const branches = [branch("fix/one"), branch("origin/fix/one", { kind: "remote" })];
    const all = buildRefTree(input({ branches }));
    const localFolder = all.find((node) => node.kind === "folder" && node.depth === 1);
    expect(localFolder).toBeDefined();

    const folded = flatten(all, new Set([localFolder?.id ?? ""]));

    expect(folded.length).toBeLessThan(all.length);
    expect(folded.some((node) => node.id.startsWith("remote:"))).toBe(true);
  });
});

describe("what is ticked when a repository is opened", () => {
  const tree = () =>
    buildRefTree(
      input({
        branches: [
          branch("master", { isHead: true }),
          branch("feature/a"),
          branch("origin/master", {
            kind: "remote",
            fullName: "refs/remotes/origin/master",
          }),
          branch("origin/feature/a", {
            kind: "remote",
            fullName: "refs/remotes/origin/feature/a",
          }),
        ],
        tags: [tag("v1.0")],
      }),
    );

  // The graph showed master and the current branch while every box looked cleared: the
  // ticks and the walk were reading different defaults.
  it("ticks HEAD and nothing else; other branches are labels until ticked", () => {
    expect([...defaultVisible(tree())]).toEqual(["HEAD"]);
  });

  it("leaves tags alone: a tag is a label, not a line of history to draw", () => {
    expect(defaultVisible(tree()).has("tag:v1.0")).toBe(false);
  });

  it("ticks nothing structural, so a group box stays a summary of its children", () => {
    const ticked = defaultVisible(tree());
    expect([...ticked].some((id) => id.startsWith("group:"))).toBe(false);
    expect([...ticked].some((id) => id.startsWith("folder:"))).toBe(false);
  });

  it("walks from exactly what is ticked", () => {
    const nodes = tree();
    expect(visibleTips(nodes, defaultVisible(nodes))).toEqual(["HEAD"]);
  });
});
