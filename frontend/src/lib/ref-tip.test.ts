import { describe, expect, it } from "vitest";
import type { CommitDetails } from "$lib/ipc";
import type { RefNode } from "./ref-nodes";
import { refTip, rowTip } from "./ref-tip";

const at = Date.UTC(2026, 8, 18, 12, 36) / 1000;
const details: CommitDetails = {
  oid: "2b7ccef0123456789abcdef0123456789abcdef",
  parents: [],
  summary: "Merge remote-tracking branch 'origin/master'",
  body: "",
  author: { name: "Ann", email: "ann@x.org", timestamp: at, tzOffsetMinutes: 180 },
  committer: { name: "Bob", email: "bob@x.org", timestamp: at + 60, tzOffsetMinutes: 180 },
  notes: [],
  trailers: [],
  signed: false,
  encoding: null,
};

const local = {
  id: "local:feature",
  kind: "local",
  label: "feature",
  depth: 1,
  oid: details.oid,
  branch: { name: "feature/x", upstream: "origin/feature/x", ahead: 0, behind: 0 },
} as unknown as RefNode;

describe("refTip", () => {
  it("says what a local branch tracks and the commit it is on", () => {
    expect(refTip(local, details)).toBe(
      [
        "Local Branch: feature/x",
        "Tracked Branch: origin/feature/x",
        "Tracking State: identical",
        "Date: 2026-09-18 15:36:00 +03:00",
        "Author: Ann <ann@x.org>",
        "Commit Message: Merge remote-tracking branch 'origin/master'",
        "Commit Date: 2026-09-18 15:37:00 +03:00",
        "Committer: Bob <bob@x.org>",
        "ID: 2b7ccef",
      ].join("\n"),
    );
  });

  it("counts how a branch stands against its upstream", () => {
    const ahead = { ...local, branch: { ...local.branch, ahead: 2, behind: 1 } } as RefNode;
    expect(refTip(ahead, null)).toContain("Tracking State: 2 ahead, 1 behind");
  });

  it("names a stash and its comment", () => {
    const stash = { id: "stash:0", kind: "stash", label: "WIP on feature", depth: 1, rev: "stash@{0}", oid: details.oid } as RefNode;
    expect(refTip(stash, details)?.split("\n").slice(0, 2)).toEqual(["Stash: stash@{0}", "Comment: WIP on feature"]);
  });

  it("describes the commit an other ref points at", () => {
    const other = { id: "other:refs/backup/x", kind: "other", label: "backup/x", depth: 1, rev: "refs/backup/x", oid: details.oid } as RefNode;
    expect(refTip(other, null)).toBe("refs/backup/x");
    expect(refTip(other, details)?.split("\n")).toEqual([
      "Date: 2026-09-18 15:36:00 +03:00",
      "Author: Ann <ann@x.org>",
      "Commit Message: Merge remote-tracking branch 'origin/master'",
      "Commit Date: 2026-09-18 15:37:00 +03:00",
      "Committer: Bob <bob@x.org>",
      "ID: 2b7ccef",
    ]);
  });

  it("has nothing to say of a folder", () => {
    expect(refTip({ id: "f", kind: "folder", label: "feature", depth: 0 }, null)).toBeNull();
  });
});

describe("rowTip", () => {
  it("is the commit a graph label sits on", () => {
    expect(
      rowTip({ oid: details.oid, summary: "Fix", authorName: "Ann", authorEmail: "ann@x.org", timestamp: at, tzOffsetMinutes: 180 }),
    ).toBe(["Date: 2026-09-18 15:36:00 +03:00", "Author: Ann <ann@x.org>", "Commit Message: Fix", "ID: 2b7ccef"].join("\n"));
  });
});
