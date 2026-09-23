import { describe, expect, it } from "vitest";
import type { BlameTables, OriginLine } from "$lib/ipc/investigate";
import {
  UNCOMMITTED,
  ageOf,
  blockAt,
  changeBlocks,
  dateOf,
  languageOf,
  markerOf,
  nextChange,
  originQuery,
  shortAuthor,
} from "./blame";

function commit(oid: string, merge = false, uncommitted = false) {
  return { oid, summary: oid, author: "A", email: "a@x", timestamp: 0, merge, boundary: false, uncommitted };
}

function line(n: number, source: number, origLine: number, change: OriginLine["change"] = "added"): OriginLine {
  return { line: n, text: `line ${n}`, source, origLine, change };
}

/** Lines 1–2 from c1 (one piece), 3–4 from c2 but not consecutive in it, 5 from a merge. */
const tables: BlameTables = {
  commits: [commit("c1"), commit("c2"), commit("m", true), commit(UNCOMMITTED, false, true)],
  sources: [
    { commit: 0, path: "a.rs", previous: { oid: "p1", path: "old.rs" } },
    { commit: 1, path: "a.rs", previous: null },
    { commit: 2, path: "a.rs", previous: null },
    { commit: 3, path: "a.rs", previous: null },
  ],
  lines: [
    line(1, 0, 10),
    line(2, 0, 11, "modified"),
    line(3, 1, 4),
    line(4, 1, 9),
    line(5, 2, 5, "modified"),
    line(6, 3, 6),
  ],
};

describe("Investigate blame helpers", () => {
  it("marks added, modified and merge lines the way SmartGit does", () => {
    expect(markerOf(tables, tables.lines[0]!)).toBe("+");
    expect(markerOf(tables, tables.lines[1]!)).toBe("~");
    expect(markerOf(tables, tables.lines[4]!)).toBe("M~");
  });

  it("writes ages compactly", () => {
    const now = 1_000_000_000;
    expect(ageOf(now - 3 * 3600, now)).toBe("3h");
    expect(ageOf(now - 140 * 86400, now)).toBe("140d");
    expect(ageOf(now - 2000 * 86400, now)).toBe("5y");
    expect(ageOf(now + 50, now)).toBe("0h");
  });

  it("writes a date as the local calendar day", () => {
    const local = new Date(2026, 8, 3, 14, 30);
    expect(dateOf(local.getTime() / 1000)).toBe("2026-09-03");
  });

  it("shortens an author to the first name", () => {
    expect(shortAuthor("Alexander Ponomarev")).toBe("Alexander");
    expect(shortAuthor("Maximilianus Long")).toBe("Maximilia…");
    expect(shortAuthor("  ")).toBe("?");
  });

  it("finds the same maximal block from any line inside it", () => {
    expect(blockAt(tables, 0)).toEqual({ start: 0, end: 2 });
    expect(blockAt(tables, 1)).toEqual({ start: 0, end: 2 });
    expect(blockAt(tables, 2), "not consecutive in the source").toEqual({ start: 2, end: 3 });
  });

  it("asks for the block in the numbering of the commit that wrote it", () => {
    expect(originQuery(tables, 1)).toEqual({
      commit: "c1",
      path: "a.rs",
      from: 10,
      to: 11,
      line: 11,
      previous: { oid: "p1", path: "old.rs" },
    });
    expect(originQuery(tables, 99)).toBeNull();
  });

  it("finds the lines the viewed version itself changed", () => {
    expect(changeBlocks(tables, "c2")).toEqual([{ start: 2, end: 4 }]);
    expect(changeBlocks(tables, null)).toEqual([{ start: 5, end: 6 }]);
  });

  it("steps to the next and previous change", () => {
    const blocks = [
      { start: 2, end: 4 },
      { start: 8, end: 9 },
    ];
    expect(nextChange(blocks, 0, 1)).toBe(2);
    expect(nextChange(blocks, 3, 1)).toBe(8);
    expect(nextChange(blocks, 8, 1)).toBeNull();
    expect(nextChange(blocks, 8, -1)).toBe(2);
    expect(nextChange(blocks, 3, -1)).toBeNull();
  });

  it("picks a bundled grammar by extension", () => {
    expect(languageOf("src/main.rs")).toBe("rust");
    expect(languageOf("App.SVELTE")).toBe("html");
    expect(languageOf("Makefile")).toBeNull();
    expect(languageOf("notes.lua")).toBeNull();
  });
});
