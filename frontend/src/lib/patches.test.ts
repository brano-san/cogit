import { describe, expect, it } from "vitest";
import { defaultMode, mailSubjects, modeProblem, patchFileName, patchKind, statRows } from "./patches";

const SHA = "3f786850e387550fdab836ed7e6dc881de23001b";

const DIFF = ["diff --git a/f.txt b/f.txt", "--- a/f.txt", "+++ b/f.txt", "@@ -1 +1 @@", "-a", "+b", ""].join("\n");

const MAILBOX = [
  `From ${SHA} Mon Sep 17 00:00:00 2001`,
  "From: A U Thor <author@example.com>",
  "Date: Sun, 15 Nov 2026 12:00:00 +0300",
  "Subject: [PATCH 1/2] Fix the parser when the input",
  " ends early",
  "",
  "Body text.",
  "---",
  DIFF,
  `From ${SHA} Mon Sep 17 00:00:00 2001`,
  "From: A U Thor <author@example.com>",
  "Subject: [PATCH 2/2] Second",
  "",
  "Subject: not a header, part of the body",
  "---",
  DIFF,
].join("\n");

describe("patch kind", () => {
  it("tells a format-patch mailbox by its From line", () => {
    expect(patchKind(MAILBOX)).toBe("mailbox");
  });

  it("tells a mail without the From line by its headers", () => {
    const mail = ["From: A <a@b>", "Subject: [PATCH] One", "", "---", DIFF].join("\n");
    expect(patchKind(mail)).toBe("mailbox");
  });

  it("takes a diff for a plain diff, CRLF too", () => {
    expect(patchKind(DIFF)).toBe("diff");
    expect(patchKind(DIFF.replaceAll("\n", "\r\n"))).toBe("diff");
  });

  it("takes a binary-only diff for a diff", () => {
    const binary = ["diff --git a/x.bin b/x.bin", "new file mode 100644", "GIT binary patch", "literal 3", "Kc", ""].join("\n");
    expect(patchKind(binary)).toBe("diff");
  });

  it("refuses text without a diff", () => {
    expect(patchKind("https://example.com/repo.git")).toBeNull();
    expect(patchKind("--- just a line\n")).toBeNull();
    expect(patchKind("")).toBeNull();
  });
});

describe("mailbox subjects", () => {
  it("unfolds a subject and drops the PATCH prefix, headers only", () => {
    expect(mailSubjects(MAILBOX)).toEqual(["Fix the parser when the input ends early", "Second"]);
  });
});

describe("mode", () => {
  it("defaults to am for a mailbox and the working tree for a diff", () => {
    expect(defaultMode("mailbox")).toBe("mailbox");
    expect(defaultMode("diff")).toBe("workTree");
  });

  it("refuses commits from a plain diff, allows a mailbox as a diff", () => {
    expect(modeProblem("diff", "mailbox")).not.toBeNull();
    expect(modeProblem("mailbox", "index")).toBeNull();
    expect(modeProblem(null, "workTree")).not.toBeNull();
  });
});

describe("preview rows", () => {
  it("counts lines, marks binaries and renames, adds up a file patched twice", () => {
    const rows = statRows([
      { path: "f.txt", oldPath: null, added: 1, removed: 2 },
      { path: "x.bin", oldPath: null, added: null, removed: null },
      { path: "new.txt", oldPath: "old.txt", added: 0, removed: 0 },
      { path: "f.txt", oldPath: null, added: 3, removed: 0 },
    ]);
    expect(rows).toEqual([
      { label: "f.txt", counts: "+4 −2", binary: false },
      { label: "x.bin", counts: "binary", binary: true },
      { label: "old.txt → new.txt", counts: "+0 −0", binary: false },
    ]);
  });
});

describe("patch file name", () => {
  it("numbers the subject and dashes what git would", () => {
    expect(patchFileName("Fix the parser: don't crash!")).toBe("0001-Fix-the-parser-don-t-crash.patch");
    expect(patchFileName("v1.2..v1.3 notes", 12)).toBe("0012-v1.2.v1.3-notes.patch");
  });

  it("keeps the whole name within 64 characters", () => {
    expect(patchFileName("a".repeat(100)).length).toBe(64);
  });
});
