import { afterEach, describe, expect, it, vi } from "vitest";
import {
  columnRows,
  graphTime,
  moveColumn,
  reconcileRows,
  toggleColumn,
  visibleColumns,
} from "./graph-columns";
import { rightCells } from "./graph-row";

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
    expect(rightCells(["hash", "author"], false)).toEqual(["hash", "author"]);
  });

  it("keeps the overlap badge right after the time", () => {
    expect(rightCells(["author", "time", "hash"], true)).toEqual(["author", "time", "overlap", "hash"]);
  });

  it("keeps the overlap badge when the time is hidden", () => {
    expect(rightCells(["author", "hash"], true)).toContain("overlap");
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

/** The Linux report: `LANG=C`, a webview whose `Intl` has no English data or leaves the
    hour out, another `TZ`. The column must read as on Windows, so nothing goes through them. */
describe("graphTime without a usable locale", () => {
  const now = Date.UTC(2026, 8, 24, 12, 0) / 1000;
  const today = Date.UTC(2026, 8, 24, 9, 7) / 1000;
  const formats = ["relative", "date", "dateTime"] as const;
  const zone = process.env.TZ;

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    process.env.TZ = zone;
  });

  it("formats every mode the same as with a full locale", () => {
    const expected = formats.map((format) => graphTime(today, 180, now, format));
    process.env.TZ = "Pacific/Kiritimati";
    const broken = () => {
      throw new RangeError("Incorrect locale information provided");
    };
    vi.stubGlobal("Intl", { RelativeTimeFormat: broken, DateTimeFormat: broken, NumberFormat: broken });
    vi.spyOn(Date.prototype, "toLocaleTimeString").mockReturnValue("");
    vi.spyOn(Date.prototype, "toLocaleDateString").mockReturnValue("");
    vi.spyOn(Date.prototype, "toLocaleString").mockReturnValue("");
    const actual = formats.map((format) => graphTime(today, 180, now, format));
    expect(actual).toEqual(expected);
    expect(actual).toEqual(["2 hours ago", "today", "today 12:07"]);
  });
});
