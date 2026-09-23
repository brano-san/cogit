import { describe, expect, it, vi } from "vitest";

const openInvestigateWindow = vi.fn(async (_url: string, _title: string) => {});
const listRepositories = vi.fn(async () => [{ repo: 4, name: "cogit" }]);

vi.mock("$lib/ipc/investigate", () => ({ openInvestigateWindow }));
vi.mock("$lib/ipc", () => ({ listRepositories }));

const { investigateTarget, openInvestigate } = await import("./open");
const { parseInvestigate } = await import("./params");

describe("openInvestigate", () => {
  it("opens the window on the file, version and line, titled after the repository", async () => {
    await openInvestigate(4, "src/main.rs", "abc", 12);

    const [url, title] = openInvestigateWindow.mock.calls[0]!;
    expect(title).toBe("main.rs [cogit] - Investigate");
    expect(parseInvestigate(new URL(url, "http://x/").search)).toEqual({
      repo: 4,
      repoName: "cogit",
      start: { path: "src/main.rs", rev: "abc", line: 12 },
    });
  });

  it("still opens for a repository that is not in the list", async () => {
    listRepositories.mockRejectedValueOnce(new Error("gone"));
    await openInvestigate(9, "a.txt", null);
    expect(openInvestigateWindow.mock.calls.at(-1)![1]).toBe("a.txt - Investigate");
  });
});

describe("investigateTarget", () => {
  const commit = { kind: "commitVsParent", oid: "c1" } as const;

  it("takes the first added line in the commit's version", () => {
    expect(investigateTarget(commit, new Set(["i:9", "i:4", "d:2"]))).toEqual({ rev: "c1", line: 4 });
  });

  it("takes a deleted line in the version before the commit", () => {
    expect(investigateTarget(commit, new Set(["d:7"]))).toEqual({ rev: "c1^", line: 7 });
    expect(investigateTarget({ kind: "workTreeVsIndex" }, new Set(["d:3"]))).toEqual({
      rev: "HEAD",
      line: 3,
    });
  });

  it("opens the working tree without a line when nothing is selected", () => {
    expect(investigateTarget({ kind: "workTreeVsIndex" }, new Set())).toEqual({ rev: null, line: null });
    expect(investigateTarget({ kind: "commitVsCommit", a: "a", b: "b" }, new Set())).toEqual({
      rev: "b",
      line: null,
    });
  });
});
