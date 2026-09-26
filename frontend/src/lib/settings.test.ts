import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DEFAULT_SETTINGS, THEMES, merge, needsRestart } from "./settings";
import { GRAPH, LANE_WIDTH, setLaneWidth } from "./graph-geometry";

describe("merge", () => {
  it("returns the defaults for nothing stored", () => {
    expect(merge(null)).toEqual(DEFAULT_SETTINGS);
  });

  it("keeps a stored value", () => {
    expect(merge({ contextLines: 8 }).contextLines).toBe(8);
  });

  it("fills in a key the stored settings do not have", () => {
    expect(merge({ contextLines: 8 }).dateFormat).toBe(DEFAULT_SETTINGS.dateFormat);
  });

  it("drops a key that is no longer a setting", () => {
    expect(merge({ removedLongAgo: 1 } as never)).not.toHaveProperty("removedLongAgo");
  });

  it("falls back for a value of the wrong type", () => {
    expect(merge({ contextLines: "lots" } as never).contextLines).toBe(
      DEFAULT_SETTINGS.contextLines,
    );
  });

  it("clamps a context size that would make the diff unreadable", () => {
    expect(merge({ contextLines: -5 }).contextLines).toBe(0);
    expect(merge({ contextLines: 9999 }).contextLines).toBe(50);
  });

  it("clamps the lane width to something drawable", () => {
    expect(merge({ laneWidth: 1 }).laneWidth).toBeGreaterThanOrEqual(8);
    expect(merge({ laneWidth: 500 }).laneWidth).toBeLessThanOrEqual(48);
  });

  // The slider said 8px while the graph drew 12, and 41–48 could not be reached.
  it("keeps the lane width within what the graph draws, end to end", () => {
    try {
      for (const stored of [1, 8, 11, 12, 40, 41, 48, 500]) {
        const kept = merge({ laneWidth: stored }).laneWidth;
        setLaneWidth(kept);
        expect(GRAPH.laneWidth, `stored ${stored}`).toBe(kept);
      }
      expect(merge({ laneWidth: 1 }).laneWidth).toBe(LANE_WIDTH.min);
      expect(merge({ laneWidth: 500 }).laneWidth).toBe(LANE_WIDTH.max);
    } finally {
      setLaneWidth(DEFAULT_SETTINGS.laneWidth);
    }
  });

  it("rejects an unknown enum value", () => {
    expect(merge({ pullMode: "rebase-and-pray" } as never).pullMode).toBe(
      DEFAULT_SETTINGS.pullMode,
    );
  });

  it("survives stored settings that are not an object at all", () => {
    expect(merge("nonsense" as never)).toEqual(DEFAULT_SETTINGS);
  });
});

describe("needsRestart", () => {
  it("is false for settings that apply immediately", () => {
    expect(needsRestart("contextLines")).toBe(false);
    expect(needsRestart("laneWidth")).toBe(false);
  });

  it("is true for the log level, which is read once at startup", () => {
    expect(needsRestart("logLevel")).toBe(true);
  });

  it("is true for the git executable path", () => {
    expect(needsRestart("gitPath")).toBe(true);
  });
});

describe("themes", () => {
  it("offers four, from the lightest to the darkest (#24)", () => {
    expect(THEMES.map(([id]) => id)).toEqual(["light", "lightGrey", "darkGrey", "dark"]);
  });

  it("keeps every one of them from a stored file", () => {
    for (const [id] of THEMES) expect(merge({ theme: id }).theme).toBe(id);
  });

  it("gives every theme but the default its own palette in app.css", () => {
    const css = readFileSync(join(__dirname, "..", "app.css"), "utf8");
    for (const [id] of THEMES.filter(([id]) => id !== DEFAULT_SETTINGS.theme)) {
      expect(css).toContain(`:root[data-theme="${id}"]`);
    }
  });
});

describe("graph display settings (#23)", () => {
  it("draws the graph as it looked before they existed", () => {
    expect(DEFAULT_SETTINGS.graphColumns).toEqual(["author", "avatar", "time", "hash"]);
    expect(DEFAULT_SETTINGS.graphTimeFormat).toBe("date");
    expect(DEFAULT_SETTINGS.graphDensity).toBe("normal");
    expect(DEFAULT_SETTINGS.graphStripes).toBe(true);
    expect(DEFAULT_SETTINGS.graphHighlightChecked).toBe(true);
    expect(DEFAULT_SETTINGS.graphLongLinkRows).toBe(40);
  });

  it("starts every graph mode off", () => {
    expect(DEFAULT_SETTINGS.graphFirstParent).toBe(false);
    expect(DEFAULT_SETTINGS.graphBranchOfCommit).toBe(false);
    expect(DEFAULT_SETTINGS.graphAncestry).toBe(false);
    expect(DEFAULT_SETTINGS.graphCollapseMerged).toBe(false);
  });

  it("fills in the defaults for a file written before the graph keys", () => {
    const merged = merge({ theme: "light", laneWidth: 20, dateFormat: "smart" });
    const graphKeys = Object.keys(DEFAULT_SETTINGS).filter((key) => key.startsWith("graph"));
    for (const key of graphKeys as (keyof typeof DEFAULT_SETTINGS)[]) {
      expect(merged[key]).toEqual(DEFAULT_SETTINGS[key]);
    }
    expect(merged.theme).toBe("light");
  });

  it("keeps a relative graph for a file whose only date setting said relative", () => {
    expect(merge({ dateFormat: "relative" }).graphTimeFormat).toBe("relative");
    expect(merge({ dateFormat: "both" }).graphTimeFormat).toBe("date");
  });

  it("lets a stored graph time format win over the old date setting", () => {
    expect(merge({ dateFormat: "relative", graphTimeFormat: "dateTime" }).graphTimeFormat).toBe(
      "dateTime",
    );
  });

  it("falls back to the default for a value outside the allowed ones", () => {
    const merged = merge({
      graphTimeFormat: "iso",
      graphDensity: "huge",
      graphStripes: "yes",
      graphFirstParent: 1,
      graphColumns: "author",
    } as never);
    expect(merged.graphTimeFormat).toBe(DEFAULT_SETTINGS.graphTimeFormat);
    expect(merged.graphDensity).toBe(DEFAULT_SETTINGS.graphDensity);
    expect(merged.graphStripes).toBe(DEFAULT_SETTINGS.graphStripes);
    expect(merged.graphFirstParent).toBe(DEFAULT_SETTINGS.graphFirstParent);
    expect(merged.graphColumns).toEqual(DEFAULT_SETTINGS.graphColumns);
  });

  it("does not clamp a link threshold out of range: it goes back to the default", () => {
    for (const bad of [-1, 1001, 12.5]) {
      expect(merge({ graphLongLinkRows: bad }).graphLongLinkRows).toBe(40);
    }
    expect(merge({ graphLongLinkRows: 0 }).graphLongLinkRows).toBe(0);
    expect(merge({ graphLongLinkRows: 120 }).graphLongLinkRows).toBe(120);
  });

  it("keeps the stored column order and what is hidden", () => {
    expect(merge({ graphColumns: ["hash", "time"] }).graphColumns).toEqual(["hash", "time"]);
    expect(merge({ graphColumns: [] }).graphColumns).toEqual([]);
  });

  it("drops unknown and repeated columns without losing the rest", () => {
    expect(
      merge({ graphColumns: ["time", "committer", "time", "author"] } as never).graphColumns,
    ).toEqual(["time", "author"]);
  });

  it("never hands out the default column list itself", () => {
    const merged = merge(null);
    merged.graphColumns.push("hash");
    expect(DEFAULT_SETTINGS.graphColumns).toEqual(["author", "avatar", "time", "hash"]);
  });
});

describe("graphFilterFields", () => {
  it("defaults to every switch but Name and Content", () => {
    expect(DEFAULT_SETTINGS.graphFilterFields).toEqual(["author", "committer", "message", "refs", "id"]);
  });

  it("keeps the switches a file saved, known ones only", () => {
    expect(merge({ graphFilterFields: ["content", "nonsense", "id"] } as never).graphFilterFields).toEqual([
      "id",
      "content",
    ]);
    expect(merge({ graphFilterFields: "id" } as never).graphFilterFields).toEqual(DEFAULT_SETTINGS.graphFilterFields);
  });

  it("never lends the defaults' own list to a merged copy", () => {
    merge({}).graphFilterFields.push("content");
    expect(DEFAULT_SETTINGS.graphFilterFields).not.toContain("content");
  });
});
