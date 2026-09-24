import { describe, expect, it } from "vitest";
import {
  columnOrder,
  columnRows,
  graphTime,
  moveColumn,
  overlapOrder,
  reconcileRows,
  toggleColumn,
  visibleColumns,
} from "./graph-columns";

describe("the column editor", () => {
  it("lists the visible columns in order, then the hidden ones", () => {
    expect(columnRows(["time", "author"])).toEqual([
      { id: "time", shown: true },
      { id: "author", shown: true },
      { id: "hash", shown: false },
      { id: "avatar", shown: false },
    ]);
  });

  it("hides and shows a column where it stands", () => {
    const rows = toggleColumn(columnRows(["author", "avatar", "time", "hash"]), "avatar");
    expect(visibleColumns(rows)).toEqual(["author", "time", "hash"]);
    expect(visibleColumns(toggleColumn(rows, "avatar"))).toEqual([
      "author",
      "avatar",
      "time",
      "hash",
    ]);
  });

  it("moves a row, and a move past either end stops at the end", () => {
    const rows = columnRows(["author", "avatar", "time", "hash"]);
    expect(visibleColumns(moveColumn(rows, 3, 0))).toEqual(["hash", "author", "avatar", "time"]);
    expect(visibleColumns(moveColumn(rows, 0, -1))).toEqual(["author", "avatar", "time", "hash"]);
    expect(visibleColumns(moveColumn(rows, 0, 9))).toEqual(["avatar", "time", "hash", "author"]);
  });

  it("keeps a hidden column where it was put while the visible ones agree", () => {
    const shown = toggleColumn(columnRows(["author", "time"]), "hash");
    const hidden = toggleColumn(moveColumn(shown, 2, 0), "hash");
    expect(hidden[0]).toEqual({ id: "hash", shown: false });
    expect(reconcileRows(hidden, visibleColumns(hidden))).toEqual(hidden);
  });

  it("starts over from the setting once it changed elsewhere", () => {
    const rows = columnRows(["author", "time"]);
    expect(reconcileRows(rows, ["hash"])).toEqual(columnRows(["hash"]));
    expect(reconcileRows([], ["hash"])).toEqual(columnRows(["hash"]));
  });
});

describe("column order in the commit list", () => {
  it("puts the columns after the subject in the chosen order", () => {
    const columns = ["hash", "author"] as const;
    expect(columnOrder(columns, "hash")).toBeLessThan(columnOrder(columns, "author"));
    expect(columnOrder(columns, "hash")).toBeGreaterThan(0);
  });

  it("keeps the overlap badge right after the time", () => {
    const columns = ["author", "time", "hash"] as const;
    const overlap = overlapOrder(columns);
    expect(overlap).toBeGreaterThan(columnOrder(columns, "time"));
    expect(overlap).toBeLessThan(columnOrder(columns, "hash"));
  });

  it("puts the overlap badge last when the time is hidden", () => {
    const columns = ["author", "hash"] as const;
    expect(overlapOrder(columns)).toBeGreaterThan(columnOrder(columns, "hash"));
  });
});

describe("graphTime", () => {
  // 2026-09-24 12:00 UTC, a Thursday.
  const now = Date.UTC(2026, 8, 24, 12, 0) / 1000;
  const yesterday = Date.UTC(2026, 8, 23, 14, 5) / 1000;
  const old = Date.UTC(2026, 8, 9, 8, 30) / 1000;

  it("says how long ago for relative", () => {
    expect(graphTime(yesterday, 0, now, "relative")).toBe("21 hours ago");
  });

  it("shows the date the column always showed for date", () => {
    expect(graphTime(yesterday, 0, now, "date")).toBe("yesterday");
    expect(graphTime(old, 0, now, "date")).toBe("09-09-26");
  });

  it("adds the clock time in the commit's own timezone for dateTime", () => {
    expect(graphTime(yesterday, 0, now, "dateTime")).toBe("yesterday 14:05");
    expect(graphTime(old, 120, now, "dateTime")).toBe("09-09-26 10:30");
  });
});
