import { describe, expect, it } from "vitest";
import type { FileEntry, FileStatus } from "./ipc";
import { contentNotice, showsWholeFile, statusFor } from "./file-content";

const entry = (path: string, status: FileStatus) => ({ path, status }) as FileEntry;
const lists = {
  unstaged: [entry("a.ts", "unchanged"), entry("b.log", "ignored"), entry("c.ts", "modified")],
  staged: [entry("c.ts", "added")],
  commit: [entry("d.ts", "unchanged")],
};

// An unchanged file showed "No change in this file."; it shows the file now.
describe("showsWholeFile", () => {
  it("shows the file where there is nothing to compare", () => {
    for (const status of ["unchanged", "ignored", "assumeUnchanged", "skipped"] as const) {
      expect(showsWholeFile(status)).toBe(true);
    }
  });

  it("keeps the diff for a change", () => {
    for (const status of ["modified", "added", "untracked", "deleted", "conflicted", "renamed"] as const) {
      expect(showsWholeFile(status)).toBe(false);
    }
    expect(showsWholeFile(null)).toBe(false);
  });
});

describe("statusFor", () => {
  it("reads the list the spec shows", () => {
    expect(statusFor({ kind: "workTreeVsIndex" }, "b.log", lists)).toBe("ignored");
    expect(statusFor({ kind: "indexVsHead" }, "c.ts", lists)).toBe("added");
    expect(statusFor({ kind: "commitVsParent", oid: "x" }, "d.ts", lists)).toBe("unchanged");
  });

  it("knows nothing of a spec with no list or a path not in it", () => {
    expect(statusFor({ kind: "commitVsWorkTree", oid: "x" }, "a.ts", lists)).toBeNull();
    expect(statusFor({ kind: "workTreeVsIndex" }, "z.ts", lists)).toBeNull();
    expect(statusFor(null, "a.ts", lists)).toBeNull();
  });
});

describe("contentNotice", () => {
  it("names a binary file by its size", () => {
    expect(contentNotice({ kind: "binary", size: 2048 })).toBe("Binary file, 2,048 bytes");
  });

  it("says how far past the limit a large file is", () => {
    expect(contentNotice({ kind: "tooLarge", size: 1_500_000, limit: 1_000_000 })).toBe(
      "The file is 1,500,000 bytes, over the limit of 1,000,000 bytes for showing it.",
    );
  });
});
