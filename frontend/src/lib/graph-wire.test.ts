import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { decodeBase64Window, decodeWindow } from "./graph-wire";

/** Written by `crates/app_state/tests/graph_wire.rs` from the window described there. */
function golden(): ArrayBuffer {
  const file = readFileSync(fileURLToPath(new URL("./graph-wire.golden.bin", import.meta.url)));
  return file.buffer.slice(file.byteOffset, file.byteOffset + file.byteLength);
}

describe("graph window format", () => {
  it("reads the window the Rust side wrote", () => {
    const block = decodeWindow(golden())!;

    expect([block.start, block.total, block.complete, block.length]).toEqual([40, 1000, false, 2]);
    expect(block.entry(0)).toEqual({
      commit: {
        oid: "a".repeat(40),
        summary: "Merge branch 'feature'",
        authorName: "Ann",
        authorEmail: "ann@example.com",
        timestamp: 1_700_000_000,
        tzOffsetMinutes: 180,
      },
      layout: {
        row: 40,
        lane: 0,
        color: 0,
        kind: "merge",
        primary: true,
        width: 2,
        segments: [
          { from: 0, to: 0, span: "bottom", primary: true, color: 0, arrow: false },
          { from: 0, to: 1, span: "bottom", primary: false, color: 0, arrow: false },
        ],
        links: [],
      },
    });
  });

  it("keeps text that is not ASCII and time zones west of Greenwich", () => {
    const row = decodeWindow(golden())!.entry(1);

    expect(row.commit.summary).toBe("Übersicht — 修正");
    expect(row.commit.tzOffsetMinutes).toBe(-300);
    expect(row.layout).toMatchObject({ row: 41, lane: 1, color: 3, kind: "root", primary: false });
    expect(row.layout.segments).toEqual([{ from: 1, to: 1, span: "top", primary: false, color: 1, arrow: true }]);
    expect(row.layout.links).toEqual([{ segment: 0, oid: "c".repeat(40) }]);
  });

  it("decodes a row once, however often it is asked for", () => {
    const block = decodeWindow(golden())!;

    expect(block.entry(1)).toBe(block.entry(1));
    expect(block.oid(1)).toBe("b".repeat(40));
  });

  it("finds a row by its whole object id only", () => {
    const block = decodeWindow(golden())!;

    expect(block.find("b".repeat(40))).toBe(1);
    expect(block.find("b".repeat(7))).toBe(-1);
    expect(block.find("a".repeat(20) + "b".repeat(20))).toBe(-1);
    expect(block.find("c".repeat(40)), "the far end of a link is not a row here").toBe(-1);
  });

  it("reads an empty buffer as a window of a replaced graph", () => {
    expect(decodeWindow(new ArrayBuffer(0))).toBeNull();
    expect(decodeBase64Window("")).toBeNull();
  });

  it("reads the window from the base64 text it crosses the bridge as", () => {
    const text = Buffer.from(golden()).toString("base64");

    expect(decodeBase64Window(text)!.entry(1)).toEqual(decodeWindow(golden())!.entry(1));
  });
});
