import { describe, expect, it } from "vitest";
import { folderDeletion, deletionQuestion } from "./folder-delete";
import { buildRefTree, type RefNode } from "./ref-nodes";
import type { Branch, Tag } from "$lib/ipc";

const branch = (name: string, kind: "local" | "remote", isHead = false): Branch =>
  ({
    name,
    fullName: `${kind === "local" ? "refs/heads" : "refs/remotes"}/${name}`,
    kind,
    isHead,
    oid: "a".repeat(40),
    upstream: null,
    ahead: 0,
    behind: 0,
  }) as unknown as Branch;

const tag = (name: string): Tag =>
  ({ name, fullName: `refs/tags/${name}`, oid: "b".repeat(40), isAnnotated: false, pointsToCommit: true }) as unknown as Tag;

const tree = buildRefTree({
  head: { kind: "branch", name: "fix/cur", oid: "a".repeat(40) } as never,
  branches: [
    branch("fix/a", "local"),
    branch("fix/b", "local"),
    branch("fix/cur", "local", true),
    branch("solo", "local"),
    branch("origin/fix/a", "remote"),
    branch("origin/fix/b", "remote"),
    branch("origin/HEAD", "remote"),
  ],
  tags: [tag("v/1"), tag("v/2")],
  stashes: [],
  lost: [],
  remoteUrls: {},
  remotes: ["origin"],
  collapsed: new Set(),
  filter: "",
});
const folder = (id: string): RefNode => tree.find((node) => node.id === id) as RefNode;

describe("folderDeletion", () => {
  it("lists a local folder and leaves out the current branch with the reason", () => {
    const plan = folderDeletion(tree, folder("folder:local/fix"));
    expect(plan?.kind).toBe("branch");
    expect(plan?.names).toEqual(["fix/a", "fix/b"]);
    expect(plan?.skipped).toEqual([{ name: "fix/cur", reason: "it is the current branch" }]);
  });

  it("knows a remote folder's remote and skips its HEAD pointer", () => {
    const plan = folderDeletion(tree, folder("folder:origin/fix"));
    expect(plan).toMatchObject({ kind: "remoteBranch", remote: "origin", names: ["origin/fix/a", "origin/fix/b"] });
  });

  it("lists a tag folder", () => {
    const plan = folderDeletion(tree, folder("folder::tags/v"));
    expect(plan).toMatchObject({ kind: "tag", names: ["v/1", "v/2"] });
  });

  it("names the count and what is left out in the question", () => {
    const plan = folderDeletion(tree, folder("folder:local/fix"));
    const text = deletionQuestion("fix", plan!);
    expect(text).toContain("2 branches");
    expect(text).toContain("fix/cur");
  });
});
