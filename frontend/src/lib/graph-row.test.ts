import { describe, expect, it } from "vitest";
import { textX } from "./graph-geometry";
import {
  COLUMN_WIDTH,
  GRAPH_COLUMNS,
  graphClipX,
  graphTime,
  rightCells,
  rightColumnsWidth,
  rowTextX,
  timeWidth,
} from "./graph-row";

const spec = { columns: GRAPH_COLUMNS, avatars: true, time: "smart" as const, overlap: false, gap: 6, padding: 12 };

describe("the right columns", () => {
  it("are in the order they always were until Preferences say otherwise", () => {
    expect(GRAPH_COLUMNS).toEqual(["author", "avatar", "time", "hash"]);
    expect(rightCells(GRAPH_COLUMNS, true)).toEqual(["author", "avatar", "time", "overlap", "hash"]);
  });

  it("keep the overlap cell when the time is hidden", () => {
    expect(rightCells(["hash", "author"], true)).toEqual(["overlap", "hash", "author"]);
  });

  it("take their widths, a gap before each and the row's padding", () => {
    const width = rightColumnsWidth(spec);

    expect(width).toBe(
      COLUMN_WIDTH.author + COLUMN_WIDTH.avatar + timeWidth("smart") + COLUMN_WIDTH.hash + 4 * 6 + 12,
    );
  });

  it("take no room for an avatar that is not drawn or a column that is hidden", () => {
    const all = rightColumnsWidth(spec);

    expect(rightColumnsWidth({ ...spec, avatars: false })).toBe(all - COLUMN_WIDTH.avatar - 6);
    expect(rightColumnsWidth({ ...spec, columns: ["author", "avatar", "time"] })).toBe(all - COLUMN_WIDTH.hash - 6);
  });

  it("make room for a longer time format", () => {
    expect(rightColumnsWidth({ ...spec, time: "both" })).toBeGreaterThan(rightColumnsWidth(spec));
    expect(timeWidth("date")).toBeLessThan(timeWidth("dateTime"));
  });
});

describe("the graph area", () => {
  it("is not cut while the subject has its room", () => {
    const clip = graphClipX(1200, 300, 160);

    expect(rowTextX(12, clip)).toBe(textX(12));
  });

  it("is cut at its edge, lanes unsqueezed, once the right columns would be pushed out", () => {
    const clip = graphClipX(700, 300, 160);

    expect(clip).toBe(700 - 300 - 160);
    expect(rowTextX(30, clip)).toBe(clip);
    expect(rowTextX(2, clip)).toBe(textX(2));
  });

  it("always keeps one lane", () => {
    expect(graphClipX(200, 300, 160)).toBe(textX(1));
  });
});

describe("graphTime", () => {
  const noon = Date.UTC(2026, 8, 24, 12, 5) / 1000;
  const now = noon + 3 * 86_400;

  it("writes the formats the graph offers", () => {
    expect(graphTime(noon, 0, now, "relative")).toBe("3 days ago");
    expect(graphTime(noon, 0, now, "date")).toBe("24-09-26");
    expect(graphTime(noon, 180, now, "dateTime")).toBe("24-09-26 15:05");
  });

  it("falls back to the app's date format, as the list did before", () => {
    expect(graphTime(noon, 0, now, "smart")).toBe("Thursday");
  });
});
