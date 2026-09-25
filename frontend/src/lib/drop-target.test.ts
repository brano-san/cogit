import { describe, expect, it } from "vitest";
import {
  DRAG_THRESHOLD,
  dropActions,
  dropOn,
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
  it("round-trips a branch", () => {
    expect(parseDrag(serialiseDrag({ kind: "branch", id: "feature" }))).toEqual({
      kind: "branch",
      id: "feature",
    });
  });

  it("round-trips a commit", () => {
    expect(parseDrag(serialiseDrag({ kind: "commit", id: "abc123" }))).toEqual({
      kind: "commit",
      id: "abc123",
    });
  });

  it("returns null for anything else the browser hands over", () => {
    expect(parseDrag("some text a user dragged in")).toBeNull();
    expect(parseDrag("")).toBeNull();
    expect(parseDrag('{"kind":"nonsense","id":"x"}')).toBeNull();
  });

  it("returns null for a payload missing an id", () => {
    expect(parseDrag('{"kind":"branch"}')).toBeNull();
  });
});

describe("dropActions", () => {
  const branch = { kind: "branch", id: "feature" } as const;
  const other = { kind: "branch", id: "main" } as const;
  const commit = { kind: "commit", id: "abc" } as const;

  it("offers merge and rebase when a branch lands on a branch", () => {
    const ids = dropActions(branch, other, false).map((a) => a.id);
    expect(ids).toContain("merge");
    expect(ids).toContain("rebase");
  });

  it("offers fast-forward only when it is actually possible", () => {
    expect(dropActions(branch, other, false).map((a) => a.id)).not.toContain("fastForward");
    expect(dropActions(branch, other, true).map((a) => a.id)).toContain("fastForward");
  });

  it("offers nothing when a branch lands on itself", () => {
    expect(dropActions(branch, branch, true)).toEqual([]);
  });

  it("offers squash when a commit lands on another commit", () => {
    const ids = dropActions(commit, { kind: "commit", id: "def" }, false).map((a) => a.id);
    expect(ids).toContain("squash");
  });

  it("offers nothing when a commit lands on itself", () => {
    expect(dropActions(commit, commit, false)).toEqual([]);
  });

  it("offers nothing for mixed kinds, which has no meaning", () => {
    expect(dropActions(commit, branch, false)).toEqual([]);
    expect(dropActions(branch, commit, false)).toEqual([]);
  });

  it("labels every action with the names involved", () => {
    for (const action of dropActions(branch, other, true)) {
      expect(action.title).toContain("feature");
      expect(action.title).toContain("main");
    }
  });
});

// Merge and rebase run on the checked-out branch. With `dev` checked out, "Merge feature
// into main" made a merge commit in dev, and "Rebase feature onto main" rebased dev.
describe("a drop on branches that are not checked out", () => {
  const feature = { kind: "branch", id: "feature" } as const;
  const main = { kind: "branch", id: "main" } as const;
  const find = (head: string | null, id: string) =>
    dropActions(feature, main, false, head).find((action) => action.id === id);

  it("offers the merge only into the checked-out branch", () => {
    expect(find("dev", "merge")?.disabled).toBeTruthy();
    expect(find("main", "merge")?.disabled).toBeUndefined();
  });

  it("offers the rebase only of the checked-out branch", () => {
    expect(find("dev", "rebase")?.disabled).toBeTruthy();
    expect(find("feature", "rebase")?.disabled).toBeUndefined();
  });
});
