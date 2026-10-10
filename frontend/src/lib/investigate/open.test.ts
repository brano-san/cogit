import { describe, expect, it, vi } from "vitest";

const openInvestigateWindow = vi.fn(async (_url: string, _title: string) => {});
const listRepositories = vi.fn(async () => [{ repo: 4, name: "cogit" }]);

vi.mock("$lib/ipc/investigate", () => ({ openInvestigateWindow }));
vi.mock("$lib/ipc", () => ({ listRepositories }));

const { investigateRev, openInvestigate } = await import("./open");
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

describe("investigateRev", () => {
  it("opens a commit's diff on the commit's version, the working tree on the file", () => {
    expect(investigateRev({ kind: "commitVsParent", oid: "c1" })).toBe("c1");
    expect(investigateRev({ kind: "commitVsCommit", a: "a", b: "b" })).toBe("b");
    expect(investigateRev({ kind: "workTreeVsIndex" })).toBeNull();
  });
});
