import { describe, expect, it } from "vitest";
import {
  type DragPayload,
  type DropContext,
  DRAG_THRESHOLD,
  dropActions,
  dropOn,
  ghostAt,
  graphDropTarget,
  moveDrag,
  parseDrag,
  pressDrag,
  serialiseDrag,
} from "./drop-target";

// WebView2 hands HTML5 drag-and-drop to the native drop target while Tauri's file drop is
// on, so `drop` never reached a row and the Merge/Rebase/Squash menu never opened. Rows
// are dragged on pointer events now: press, move past the threshold, release over a row.
describe("a drag on pointer events", () => {
  it("stays a click until the pointer moves past the threshold", () => {
    const pressed = pressDrag("feature", 100, 50);
    const nudged = moveDrag(pressed, 100 + DRAG_THRESHOLD, 50);
    expect(nudged.moving).toBe(false);
    expect(dropOn(nudged, "main")).toBeNull();
  });

  it("drops on the row it is released over once it has moved", () => {
    const drag = moveDrag(pressDrag("feature", 100, 50), 100, 50 + DRAG_THRESHOLD + 1);
    expect(drag.moving).toBe(true);
    expect(dropOn(drag, "main")).toBe("main");
  });

  it("keeps moving after coming back near where it started", () => {
    const away = moveDrag(pressDrag("feature", 0, 0), 40, 0);
    expect(moveDrag(away, 1, 0).moving).toBe(true);
  });

  it("drops nowhere off every row, or back on its own", () => {
    const drag = moveDrag(pressDrag("feature", 0, 0), 0, 30);
    expect(dropOn(drag, null)).toBeNull();
    expect(dropOn(drag, "feature")).toBeNull();
    expect(dropOn(null, "main")).toBeNull();
  });

  it("carries the row under the pointer, held where it was grabbed", () => {
    const drag = pressDrag("feature", 130, 210);
    expect(ghostAt(drag, { left: 100, top: 200 }, 130, 210)).toEqual({ left: 100, top: 200 });
    expect(ghostAt(drag, { left: 100, top: 200 }, 150, 290)).toEqual({ left: 120, top: 280 });
  });
});

describe("graphDropTarget", () => {
  const rowHeight = 22;
  const oids = ["c0", "c1", "c2", "c3"];
  const oidAt = (row: number) => oids[row];
  // One Working Tree row above four commits.
  const at = (y: number, scrollTop = 0, headerRows = 1) =>
    graphDropTarget(y, scrollTop, rowHeight, oids.length + headerRows, headerRows, oidAt);

  it("is the commit of the row under the pointer", () => {
    expect(at(rowHeight * 1 + 1)).toBe("c0");
    expect(at(rowHeight * 3 + rowHeight - 1)).toBe("c2");
  });

  it("counts the rows scrolled away above the viewport", () => {
    expect(at(5, rowHeight * 2)).toBe("c1");
  });

  it("is nothing on the Working Tree row, on a rebase row or past the last commit", () => {
    expect(at(5)).toBeNull();
    expect(at(rowHeight * 1 + 5, 0, 2)).toBeNull();
    expect(at(rowHeight * 5 + 1)).toBeNull();
    expect(at(-1)).toBeNull();
  });

  it("is nothing on a row whose commit has not arrived yet", () => {
    expect(graphDropTarget(rowHeight * 2 + 1, 0, rowHeight, 10, 1, () => undefined)).toBeNull();
  });
});

describe("serialiseDrag and parseDrag", () => {
  it("round-trips every kind, names with colons and slashes too", () => {
    for (const payload of [
      { kind: "branch", id: "feature/x" },
      { kind: "remote", id: "origin/main" },
      { kind: "tag", id: "v1:rc" },
      { kind: "commit", id: "abc123" },
      { kind: "step", id: "2" },
    ] as const) {
      expect(parseDrag(serialiseDrag(payload))).toEqual(payload);
    }
  });

  it("returns null for anything that is not a payload", () => {
    expect(parseDrag("some text")).toBeNull();
    expect(parseDrag("")).toBeNull();
    expect(parseDrag("nonsense:x")).toBeNull();
    expect(parseDrag("branch:")).toBeNull();
  });
});

describe("dropActions", () => {
  const oids: Record<string, string> = {
    "branch:feature": "f1",
    "branch:main": "m1",
    "remote:origin/main": "m1",
    "tag:v1": "t1",
    "commit:c1": "c1",
    "commit:m1": "m1",
  };
  const context = (head: string | null = null, upstream: string | null = null, ff = false): DropContext => ({
    head,
    oidOf: (item) => oids[`${item.kind}:${item.id}`],
    upstreamOf: () => upstream,
    canFastForward: () => ff,
  });
  const feature = { kind: "branch", id: "feature" } as const;
  const main = { kind: "branch", id: "main" } as const;
  const remote = { kind: "remote", id: "origin/main" } as const;
  const tag = { kind: "tag", id: "v1" } as const;
  const commit = { kind: "commit", id: "c1" } as const;
  const ids = (source: DragPayload, target: DragPayload, given = context()) =>
    dropActions(source, target, given).map((action) => action.id);
  const find = (source: DragPayload, target: DragPayload, id: string, given = context()) =>
    dropActions(source, target, given).find((action) => action.id === id);

  it("offers merge and rebase for a branch on a branch, fast-forward only when it can", () => {
    expect(ids(feature, main)).toEqual(["merge", "rebase"]);
    expect(ids(feature, main, context(null, null, true))).toContain("fastForward");
  });

  it("merges into and rebases only the checked-out branch", () => {
    expect(find(feature, main, "merge", context("dev"))?.disabled).toBeTruthy();
    expect(find(feature, main, "merge", context("main"))?.disabled).toBeUndefined();
    expect(find(feature, main, "rebase", context("dev"))?.disabled).toBeTruthy();
    expect(find(feature, main, "rebase", context("feature"))?.disabled).toBeUndefined();
  });

  it("sets the upstream between a local and a remote branch, either way round", () => {
    expect(find(feature, remote, "setUpstream")).toMatchObject({ subject: "feature", object: "origin/main" });
    expect(find(remote, main, "setUpstream")).toMatchObject({ subject: "main", object: "origin/main" });
    expect(find(feature, remote, "setUpstream", context(null, "origin/main"))?.disabled).toBeTruthy();
  });

  it("moves a branch onto a commit or a tag, not where it already is", () => {
    expect(find(feature, commit, "moveBranch")).toMatchObject({ subject: "feature", object: "c1", destructive: true });
    expect(find(feature, tag, "moveBranch")?.object).toBe("t1");
    expect(find(main, { kind: "commit", id: "m1" }, "moveBranch")?.disabled).toBeTruthy();
  });

  it("moves a tag onto anything, and a commit dropped on a tag moves the tag", () => {
    expect(find(tag, commit, "moveTag")).toMatchObject({ subject: "v1", object: "c1" });
    expect(find(tag, main, "moveTag")?.object).toBe("m1");
    expect(find(tag, remote, "moveTag")?.object).toBe("m1");
    expect(find(commit, tag, "moveTag")).toMatchObject({ subject: "v1", object: "c1" });
  });

  it("cherry-picks a commit onto the checked-out branch or moves the branch to it", () => {
    expect(ids(commit, main)).toEqual(["cherryPick", "moveBranch"]);
    expect(find(commit, main, "cherryPick", context("dev"))?.disabled).toBeTruthy();
    expect(find(commit, main, "cherryPick", context("main"))?.disabled).toBeUndefined();
    expect(find(commit, main, "moveBranch")).toMatchObject({ subject: "main", object: "c1" });
  });

  it("squashes or reorders a commit on a commit", () => {
    expect(ids(commit, { kind: "commit", id: "m1" })).toEqual(["squash", "reorder"]);
  });

  it("offers nothing for a drop on itself or a pair with no meaning", () => {
    expect(ids(feature, feature)).toEqual([]);
    expect(ids(commit, commit)).toEqual([]);
    expect(ids(remote, { kind: "remote", id: "origin/dev" })).toEqual([]);
    expect(ids(remote, commit)).toEqual([]);
    expect(ids(commit, remote)).toEqual([]);
  });

  it("names both sides in every title", () => {
    for (const action of dropActions(feature, main, context(null, null, true))) {
      expect(action.title).toContain("feature");
      expect(action.title).toContain("main");
    }
  });
});
