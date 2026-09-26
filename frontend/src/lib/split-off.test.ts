import { describe, expect, it } from "vitest";
import { splitProblem, splitRequest, splitStarted, splitSummary } from "./split-off";

// The dialog read the selected commit live: a commit selected while it was open — a PageDown
// in the graph, a reveal from Blame — became the one Split Off cut, and the warning about a
// published commit stayed the old one's.
describe("splitRequest", () => {
  it("takes the files and the warning of the commit it opened on, once", async () => {
    const asked: string[] = [];
    const request = await splitRequest("c1", {
      files: async (oid) => {
        asked.push(`files ${oid}`);
        return ["a.txt", "b.txt"];
      },
      published: async (oid) => {
        asked.push(`published ${oid}`);
        return true;
      },
    });

    expect(request).toEqual({ oid: "c1", changed: ["a.txt", "b.txt"], published: true });
    expect(asked).toEqual(["files c1", "published c1"]);
  });
});

describe("splitProblem", () => {
  const files = ["a.txt", "b.txt", "c.txt"];

  it("has nothing to say about a sensible split", () => {
    expect(splitProblem(files, ["a.txt"], "split: a")).toBeNull();
  });

  it("refuses an empty selection", () => {
    expect(splitProblem(files, [], "message")).toMatch(/choose/i);
  });

  it("refuses taking every file, which is not a split", () => {
    expect(splitProblem(files, files, "message")).toMatch(/at least one/i);
  });

  it("refuses an empty message, because a commit needs one", () => {
    expect(splitProblem(files, ["a.txt"], "   ")).toMatch(/message/i);
  });

  it("refuses a commit that changed only one file", () => {
    expect(splitProblem(["only.txt"], ["only.txt"], "message")).toMatch(/at least one/i);
  });
});

describe("splitSummary", () => {
  it("says how the two commits will be divided", () => {
    expect(splitSummary(["a.txt", "b.txt", "c.txt"], ["a.txt"], true)).toBe(
      "1 file before, 2 files in the original commit",
    );
  });

  it("puts the split commit after when asked", () => {
    expect(splitSummary(["a.txt", "b.txt"], ["a.txt"], false)).toBe(
      "1 file in the original commit, 1 file after",
    );
  });

  it("uses the singular for one file", () => {
    expect(splitSummary(["a.txt", "b.txt"], ["a.txt"], true)).toContain("1 file before");
  });
});

// A click beside Split Off lost the ticked files and the typed message without a word.
describe("splitStarted", () => {
  it("is false until a file is ticked or a message typed", () => {
    expect(splitStarted([], "  ")).toBe(false);
    expect(splitStarted(["a.txt"], "")).toBe(true);
    expect(splitStarted([], "Move the docs")).toBe(true);
  });
});
