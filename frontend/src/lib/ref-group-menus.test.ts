import { describe, expect, it } from "vitest";
import type { ContextItem } from "./ipc";
import type { RefNode } from "./ref-nodes";
import {
  claimsNode,
  depthProblem,
  groupMenu,
  remoteMenu,
  remoteNameProblem,
  remotePull,
  type RemoteFacts,
} from "./ref-group-menus";

const labels = (items: ContextItem[]) => items.map((entry) => (entry.separator ? "—" : entry.label));
const off = (items: ContextItem[]) => items.filter((entry) => !entry.separator && !entry.enabled).map((entry) => entry.label);

const node = (id: string, kind: RefNode["kind"], over: Partial<RefNode> = {}): RefNode => ({
  id,
  kind,
  label: id,
  depth: 0,
  ...over,
});

describe("claimsNode", () => {
  it("takes HEAD, headings and folders; refs, stashes and lost commits keep their menus", () => {
    expect(claimsNode(node("HEAD", "head"))).toBe(true);
    expect(claimsNode(node("group:local", "group"))).toBe(true);
    expect(claimsNode(node("remote-group:origin", "group", { remote: "origin" }))).toBe(true);
    expect(claimsNode(node("folder:local/fix", "folder"))).toBe(true);
    for (const kind of ["local", "remote", "tag", "stash", "lost"] as const) {
      expect(claimsNode(node(`${kind}:x`, kind))).toBe(false);
    }
  });
});

// #19: every node has Toggle; Local Branches adds Add Branch, Tags adds Add Tag.
describe("groupMenu", () => {
  it("gives Local Branches Add Branch and Tags Add Tag, both above Toggle", () => {
    expect(labels(groupMenu(node("group:local", "group"), null))).toEqual(["Add Branch…", "—", "Toggle"]);
    expect(labels(groupMenu(node("group:tags", "group"), null))).toEqual(["Add Tag…", "—", "Toggle"]);
  });

  it("gives HEAD, Stashes, Lost Commits and folders Toggle alone", () => {
    for (const at of [node("HEAD", "head"), node("group:stashes", "group"), node("folder:local/fix", "folder")]) {
      expect(labels(groupMenu(at, null)), at.id).toEqual(["Toggle"]);
    }
  });

  it("says why Toggle is off", () => {
    expect(off(groupMenu(node("group:tags", "group"), "no tag points to a commit"))).toEqual([
      "Toggle (no tag points to a commit)",
    ]);
  });
});

const facts = (over: Partial<RemoteFacts> = {}): RemoteFacts => ({
  remote: "origin",
  configured: true,
  head: { name: "main", upstream: "origin/main" },
  upstreamRemote: "origin",
  url: "https://example.com/x.git",
  shallow: false,
  toggle: null,
  ...over,
});

describe("remoteMenu", () => {
  it("lists the remote's items in the order of the task", () => {
    expect(labels(remoteMenu(facts({ shallow: true })))).toEqual([
      "Push To…",
      "—",
      "Pull",
      "Fetch",
      "Fetch More",
      "—",
      "Rename…",
      "Delete",
      "—",
      "Copy URL",
      "—",
      "Set Depth…",
      "Properties…",
      "—",
      "Toggle",
    ]);
    expect(off(remoteMenu(facts({ shallow: true })))).toEqual([]);
  });

  it("deepens only a shallow clone", () => {
    expect(off(remoteMenu(facts()))).toEqual(["Set Depth… (not a shallow clone)"]);
  });

  it("pushes and pulls only from a branch, and pulls only the remote it tracks", () => {
    expect(off(remoteMenu(facts({ head: null, upstreamRemote: null })))).toEqual([
      "Push To… (HEAD is not on a branch)",
      "Pull (HEAD is not on a branch)",
      "Set Depth… (not a shallow clone)",
    ]);
    const other = facts({ remote: "fork", head: { name: "main", upstream: "origin/main" } });
    expect(off(remoteMenu(other))).toContain("Pull (main tracks origin/main)");
  });

  it("keeps a heading of remote branches with no remote behind them to Toggle and Copy URL", () => {
    expect(off(remoteMenu(facts({ configured: false, url: null, shallow: true })))).toEqual([
      "Push To… (not a configured remote)",
      "Pull (not a configured remote)",
      "Fetch (not a configured remote)",
      "Fetch More (not a configured remote)",
      "Rename… (not a configured remote)",
      "Delete (not a configured remote)",
      "Copy URL (no URL)",
      "Set Depth… (not a configured remote)",
      "Properties… (not a configured remote)",
    ]);
  });
});

describe("remotePull", () => {
  it("pulls the tracked remote and fetches one a branch without upstream is pulled from", () => {
    expect(remotePull(facts())).toBe("pull");
    expect(remotePull(facts({ head: { name: "topic", upstream: null }, upstreamRemote: null }))).toBe("fetch");
    expect(remotePull(facts({ remote: "fork" }))).toBeNull();
    expect(remotePull(facts({ head: null }))).toBeNull();
  });
});

describe("remoteNameProblem", () => {
  const taken = ["origin", "upstream"];

  it("wants a name git takes that no other remote has", () => {
    expect(remoteNameProblem("  ", taken, "origin")).toMatch(/Enter/);
    expect(remoteNameProblem("upstream", taken, "origin")).toBe("There is a remote upstream already.");
    for (const bad of ["two words", "a..b", "x:y", "-dash", "end/", "a.lock"]) {
      expect(remoteNameProblem(bad, taken, "origin"), bad).toBe("Git will refuse that name.");
    }
    expect(remoteNameProblem("fork", taken, "origin")).toBeNull();
    expect(remoteNameProblem("team/mirror", taken, "origin")).toBeNull();
    expect(remoteNameProblem("origin", taken, "origin")).toBeNull();
  });
});

describe("depthProblem", () => {
  it("takes a whole number of commits, one or more", () => {
    for (const bad of ["", "0", "-3", "2.5", "ten", "99999999999"]) {
      expect(depthProblem(bad), bad).not.toBeNull();
    }
    expect(depthProblem(" 50 ")).toBeNull();
  });
});
