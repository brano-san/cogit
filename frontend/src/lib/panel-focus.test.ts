import { describe, expect, it } from "vitest";
import { allowsSelectAll, settle, step } from "./panel-focus";
import { PANELS, type PanelId } from "./perspectives";

const all = () => true;
const only = (...shown: PanelId[]) => (panel: PanelId) => shown.includes(panel);

describe("step", () => {
  it("walks the panels in their laid-out order", () => {
    expect(step("repositories", all, 1)).toBe(PANELS[1]);
  });

  it("wraps round at the end", () => {
    const last = PANELS[PANELS.length - 1] as PanelId;
    expect(step(last, all, 1)).toBe(PANELS[0]);
  });

  it("wraps round backwards", () => {
    expect(step(PANELS[0] as PanelId, all, -1)).toBe(PANELS[PANELS.length - 1]);
  });

  it("skips hidden panels rather than focusing nothing", () => {
    expect(step("repositories", only("repositories", "diff"), 1)).toBe("diff");
    expect(step("diff", only("repositories", "diff"), 1)).toBe("repositories");
  });

  it("stays put when it is the only panel left", () => {
    expect(step("graph", only("graph"), 1)).toBe("graph");
    expect(step("graph", only("graph"), -1)).toBe("graph");
  });

  it("lands on the first visible panel when the current one is hidden", () => {
    expect(step("commit", only("files", "diff"), 1)).toBe("files");
  });

  it("does not move when every panel is hidden", () => {
    expect(step("graph", () => false, 1)).toBe("graph");
  });
});

describe("settle", () => {
  it("leaves a visible panel alone", () => {
    expect(settle("files", all)).toBe("files");
  });

  it("moves off a panel that has just been hidden", () => {
    expect(settle("commit", only("graph", "diff"))).toBe("graph");
  });

  it("keeps the current panel when there is nowhere to go", () => {
    expect(settle("commit", () => false)).toBe("commit");
  });
});

describe("allowsSelectAll", () => {
  it("refuses the graph on purpose", () => {
    expect(allowsSelectAll("graph")).toBe(false);
  });

  it("allows every other panel", () => {
    for (const panel of PANELS.filter((p) => p !== "graph")) {
      expect(allowsSelectAll(panel)).toBe(true);
    }
  });
});
