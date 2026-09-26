import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import { fileName, matchesMask, sortFiles, statusBadge, statusLabel, statusTooltip } from "./files";

function entry(path: string, status: FileEntry["status"] = "modified"): FileEntry {
  return { path, oldPath: null, status, mode: "plain", modeChange: null, similarity: null };
}

describe("statusBadge", () => {
  it("gives every status its own single letter", () => {
    const badges = (
      ["added", "modified", "deleted", "renamed", "copied", "untracked", "conflicted"] as const
    ).map(statusBadge);
    expect(badges).toEqual(["A", "M", "D", "R", "C", "?", "U"]);
    expect(new Set(badges).size).toBe(badges.length);
  });

  it("spells the status out for assistive technology", () => {
    expect(statusLabel("renamed")).toBe("Renamed");
  });

  it("explains every marker the list can show in its tooltip", () => {
    expect(statusTooltip("modified")).toBe("Modified — changed since last commit");
    expect(statusTooltip("untracked")).toBe("Untracked — not under version control");
    expect(statusTooltip("conflicted")).toBe("Conflict — unmerged");
    for (const status of ["unchanged", "ignored", "assumeUnchanged", "skipped"] as const) {
      expect(statusTooltip(status)).toMatch(/^[A-Z]/);
    }
  });
});

describe("fileName", () => {
  it("takes the last segment of a path", () => {
    expect(fileName("src/deep/mod.rs")).toBe("mod.rs");
  });

  it("returns the whole path when there is no directory", () => {
    expect(fileName("README.md")).toBe("README.md");
  });

  it("survives a trailing slash without returning an empty name", () => {
    expect(fileName("src/deep/")).toBe("deep");
  });
});

describe("matchesMask", () => {
  it("accepts everything when the mask is empty", () => {
    expect(matchesMask("src/main.rs", "")).toBe(true);
    expect(matchesMask("src/main.rs", "   ")).toBe(true);
  });

  it("matches an extension mask against the file name", () => {
    expect(matchesMask("src/deep/main.cpp", "*.cpp")).toBe(true);
    expect(matchesMask("src/deep/main.rs", "*.cpp")).toBe(false);
  });

  it("matches a bare substring anywhere in the path", () => {
    expect(matchesMask("src/deep/main.rs", "deep")).toBe(true);
    expect(matchesMask("src/deep/main.rs", "shallow")).toBe(false);
  });

  it("ignores case so the filter is usable while typing", () => {
    expect(matchesMask("src/Main.CPP", "*.cpp")).toBe(true);
  });

  it("treats a mask with a slash as a path mask", () => {
    expect(matchesMask("src/deep/main.rs", "src/*/main.rs")).toBe(true);
    expect(matchesMask("other/deep/main.rs", "src/*/main.rs")).toBe(false);
  });

  it("does not let a regex metacharacter in the mask escape", () => {
    expect(matchesMask("aXb.txt", "a.b.txt")).toBe(false);
    expect(matchesMask("a.b.txt", "a.b.txt")).toBe(true);
  });

  it("treats ? as exactly one character", () => {
    expect(matchesMask("a1.txt", "a?.txt")).toBe(true);
    expect(matchesMask("a12.txt", "a?.txt")).toBe(false);
  });
});

describe("sortFiles", () => {
  const files = [
    entry("z/last.txt", "added"),
    entry("a/first.txt", "deleted"),
    entry("m/middle.txt", "modified"),
  ];

  it("sorts by path without mutating the input", () => {
    const sorted = sortFiles(files);
    expect(sorted.map((f) => f.path)).toEqual(["a/first.txt", "m/middle.txt", "z/last.txt"]);
    expect(files[0]!.path).toBe("z/last.txt");
  });

  it("is stable enough to be idempotent", () => {
    const once = sortFiles(files);
    expect(sortFiles(once)).toEqual(once);
  });
});
