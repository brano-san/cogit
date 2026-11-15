import { describe, expect, it } from "vitest";
import {
  branchMergeSources,
  commitMergeSource,
  commitMergeSources,
  octopusBlocked,
  octopusMessage,
  octopusQuestion,
  type MergeSource,
} from "./octopus";

const branch = (name: string): MergeSource => ({ rev: name, name, kind: "branch" });
const local = (name: string, oid = "1", isHead = false) => ({
  name,
  fullName: `refs/heads/${name}`,
  kind: "local" as const,
  oid,
  isHead,
});
const remote = (name: string, oid = "1") => ({
  name,
  fullName: `refs/remotes/${name}`,
  kind: "remote" as const,
  oid,
  isHead: false,
});

describe("octopusMessage", () => {
  // Each expectation is what git 2.51 wrote for the same merge (fmt-merge-msg).
  it("lists the branches as git does", () => {
    expect(octopusMessage([branch("a"), branch("b"), branch("c")], "main")).toBe("Merge branches 'a', 'b' and 'c'");
    expect(octopusMessage([branch("a"), branch("b")], "master")).toBe("Merge branches 'a' and 'b'");
  });

  it("names the target unless it is main or master", () => {
    expect(octopusMessage([branch("a"), branch("b")], "dev")).toBe("Merge branches 'a' and 'b' into dev");
    expect(octopusMessage([branch("a"), branch("b")], null)).toBe("Merge branches 'a' and 'b' into HEAD");
  });

  it("groups branches, remote-tracking branches, tags and commits in git's order", () => {
    const oid = "90da7d1e73a0c8b1d49abc2478b260571fae74e4";
    const sources: MergeSource[] = [
      { rev: oid, name: oid, kind: "commit" },
      { rev: "v1", name: "v1", kind: "tag" },
      { rev: "origin/r", name: "origin/r", kind: "remote" },
      branch("a"),
      branch("b"),
    ];
    expect(octopusMessage(sources, "dev")).toBe(
      `Merge branches 'a' and 'b', remote-tracking branch 'origin/r', tag 'v1'; commit '${oid}' into dev`,
    );
    expect(octopusMessage([{ rev: "v1", name: "v1", kind: "tag" }, branch("c2")], "dev")).toBe(
      "Merge branch 'c2', tag 'v1' into dev",
    );
  });
});

describe("branchMergeSources", () => {
  const node = (kind: string, name: string, extra = {}) => ({
    kind,
    label: name.split("/").at(-1) ?? name,
    ...(kind === "tag" ? { tag: { name } } : { branch: kind === "local" ? local(name) : remote(name) }),
    ...extra,
  });

  it("takes branches and tags in order and leaves out the current branch", () => {
    const picked = branchMergeSources(
      [node("local", "main", { current: true }), node("local", "a"), node("remote", "origin/b"), node("tag", "v1")],
      [local("main"), local("a"), remote("origin/b")],
      [{ name: "v1" }],
    );
    expect(picked.sources.map((source) => [source.rev, source.kind])).toEqual([
      ["a", "branch"],
      ["origin/b", "remote"],
      ["v1", "tag"],
    ]);
    expect(picked.skipped).toEqual(["main, the current branch"]);
  });

  it("passes a branch shadowed by a tag by its full name, but names it short", () => {
    const picked = branchMergeSources([node("local", "topic")], [local("topic")], [{ name: "topic" }]);
    expect(picked.sources).toEqual([{ rev: "refs/heads/topic", name: "topic", kind: "branch" }]);
  });

  it("ignores headings, folders and origin/HEAD", () => {
    const picked = branchMergeSources(
      [{ kind: "group", label: "Local" }, node("remote", "origin/HEAD", { label: "HEAD" })],
      [],
      [],
    );
    expect(picked.sources).toEqual([]);
  });
});

describe("commit sources", () => {
  const branches = [local("main", "h", true), remote("origin/x", "x"), local("x", "x"), remote("origin/y", "y")];

  it("prefers a local branch, then a remote one, then the hash", () => {
    expect(commitMergeSource("x", branches, [])).toEqual({ rev: "x", name: "x", kind: "branch" });
    expect(commitMergeSource("y", branches, [])).toEqual({ rev: "origin/y", name: "origin/y", kind: "remote" });
    expect(commitMergeSource("z", branches, [])).toEqual({ rev: "z", name: "z", kind: "commit" });
  });

  it("leaves out HEAD's commit and counts one commit once", () => {
    const picked = commitMergeSources(["h", "x", "y", "x"], "h", (oid) => commitMergeSource(oid, branches, []));
    expect(picked.sources.map((source) => source.rev)).toEqual(["x", "origin/y"]);
    expect(picked.skipped).toHaveLength(1);
  });
});

describe("octopusBlocked and the question", () => {
  it("is off only when nothing but the current branch is selected", () => {
    expect(octopusBlocked({ sources: [], skipped: ["main, the current branch"] })).toBe("only the current branch");
    expect(octopusBlocked({ sources: [], skipped: [] })).toBe("no branch or commit selected");
    expect(octopusBlocked({ sources: [branch("a")], skipped: [] })).toBeNull();
  });

  it("lists the sources, the target and the message, and warns about conflicts", () => {
    const question = octopusQuestion({ sources: [branch("a"), branch("b")], skipped: [] }, "dev", "Merge it");
    expect(question.items).toEqual(["branch a", "branch b"]);
    expect(question.fields).toEqual([
      ["Into", "dev"],
      ["Message", "Merge it"],
    ]);
    expect(question.message).toContain("one by one");
    expect(question.message).toContain("3 parents");
  });
});
