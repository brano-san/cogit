import { describe, expect, it } from "vitest";
import type { SplitFile } from "./ipc";
import {
  addPart,
  assign,
  blockKey,
  blocksOf,
  draftStarted,
  newDraft,
  ownerOf,
  partSizes,
  removePart,
  setMessage,
  splitCommitProblem,
  splitCommitRequest,
  splitParts,
} from "./split-commit";

function file(path: string, hunks: number, whole = false): SplitFile {
  return {
    path,
    blocks: whole ? ["Whole file"] : Array.from({ length: hunks }, (_, i) => `Block ${i + 1}`),
    whole,
    note: null,
  };
}

const FILES = [file("a.ts", 2), file("b.bin", 0, true), file("c.ts", 3, true)];
const BLOCKS = blocksOf(FILES);
const A0 = blockKey("a.ts", 0);
const A1 = blockKey("a.ts", 1);
const B = blockKey("b.bin", null);
const C = blockKey("c.ts", null);

describe("blocksOf", () => {
  it("gives a text file one block per hunk and a whole file one block", () => {
    expect(BLOCKS.map((block) => [block.path, block.hunk])).toEqual([
      ["a.ts", 0],
      ["a.ts", 1],
      ["b.bin", null],
      ["c.ts", null],
    ]);
  });
});

describe("assigning blocks", () => {
  it("starts with two commits carrying the original message and every block in the last", () => {
    const draft = newDraft("fix: things");
    expect(draft.messages).toEqual(["fix: things", "fix: things"]);
    expect(partSizes(draft, BLOCKS)).toEqual([0, 4]);
  });

  it("moves blocks to the chosen commit and back to the last on a second click", () => {
    let draft = assign(newDraft("m"), [A0, B], 0);
    expect(ownerOf(draft, A0)).toBe(0);
    expect(ownerOf(draft, A1)).toBe(1);
    expect(partSizes(draft, BLOCKS)).toEqual([2, 2]);
    draft = assign(draft, [A0, B], 0);
    expect(partSizes(draft, BLOCKS)).toEqual([0, 4]);
  });

  it("re-assigns a block from one commit to another", () => {
    let draft = addPart(newDraft("m"), "third");
    draft = assign(draft, [A0], 0);
    draft = assign(draft, [A0], 1);
    expect(ownerOf(draft, A0)).toBe(1);
  });

  it("removing a commit sends its blocks to the last and renumbers the later ones", () => {
    let draft = addPart(addPart(newDraft("m"), "3"), "4");
    draft = assign(draft, [A0], 1);
    draft = assign(draft, [B], 2);
    draft = removePart(draft, 1);
    expect(draft.messages).toHaveLength(3);
    expect(ownerOf(draft, A0)).toBe(2);
    expect(ownerOf(draft, B)).toBe(1);
  });

  it("keeps at least two commits", () => {
    const draft = newDraft("m");
    expect(removePart(draft, 0)).toBe(draft);
  });
});

describe("splitCommitProblem", () => {
  it("asks for a block in every commit, the last included", () => {
    expect(splitCommitProblem(newDraft("m"), BLOCKS)).toBe("Give commit 1 at least one block.");
    const all = assign(newDraft("m"), [A0, A1, B, C], 0);
    expect(splitCommitProblem(all, BLOCKS)).toBe("Leave at least one block for commit 2.");
  });

  it("asks for a message in every commit", () => {
    const draft = setMessage(assign(newDraft("m"), [A0], 0), 1, "  ");
    expect(splitCommitProblem(draft, BLOCKS)).toBe("Commit 2 needs a message.");
  });

  it("refuses a commit with a single block", () => {
    expect(splitCommitProblem(newDraft("m"), blocksOf([file("x", 1)]))).toMatch(/nothing to split/);
  });

  it("is satisfied by a sensible split", () => {
    expect(splitCommitProblem(assign(newDraft("m"), [A0], 0), BLOCKS)).toBeNull();
  });
});

describe("splitParts", () => {
  it("names every block once, unassigned ones in the last commit", () => {
    let draft = addPart(newDraft("orig"), "third");
    draft = setMessage(assign(draft, [A1], 0), 0, " first \n");
    draft = assign(draft, [C], 1);
    expect(splitParts(draft, BLOCKS)).toEqual([
      { message: " first \n", blocks: [{ path: "a.ts", hunk: 1 }] },
      { message: "orig", blocks: [{ path: "c.ts", hunk: null }] },
      {
        message: "third",
        blocks: [
          { path: "a.ts", hunk: 0 },
          { path: "b.bin", hunk: null },
        ],
      },
    ]);
  });
});

describe("draftStarted", () => {
  it("is false until a block moves, a commit is added or a message changes", () => {
    const draft = newDraft("m");
    expect(draftStarted(draft, "m")).toBe(false);
    expect(draftStarted(assign(draft, [A0], 0), "m")).toBe(true);
    expect(draftStarted(addPart(draft, "m"), "m")).toBe(true);
    expect(draftStarted(setMessage(draft, 0, "x"), "m")).toBe(true);
  });
});

describe("splitCommitRequest", () => {
  it("reads the blocks, message and warning of the commit it opened on", async () => {
    const asked: string[] = [];
    const request = await splitCommitRequest("c1", {
      preview: async (oid) => {
        asked.push(`preview ${oid}`);
        return { files: FILES, message: "subject\n\nbody" };
      },
      published: async (oid) => {
        asked.push(`published ${oid}`);
        return false;
      },
    });
    expect(request).toEqual({ oid: "c1", message: "subject\n\nbody", files: FILES, published: false });
    expect(asked.sort()).toEqual(["preview c1", "published c1"]);
  });
});
