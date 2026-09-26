import { describe, expect, it } from "vitest";
import { clampFraction, DEFAULT_LAYOUT } from "$lib/perspectives";

describe("clampFraction", () => {
  it("leaves sensible values untouched", () => {
    expect(clampFraction(0.5)).toBe(0.5);
    expect(clampFraction(0.25)).toBe(0.25);
  });

  it("stops a pane from collapsing to nothing", () => {
    expect(clampFraction(0)).toBeGreaterThan(0);
    expect(clampFraction(-3)).toBeGreaterThan(0);
  });

  it("stops a pane from swallowing the whole container", () => {
    expect(clampFraction(1)).toBeLessThan(1);
    expect(clampFraction(50)).toBeLessThan(1);
  });

  it("survives NaN from a corrupt stored layout", () => {
    expect(Number.isFinite(clampFraction(Number.NaN))).toBe(true);
    expect(Number.isFinite(clampFraction(Number.POSITIVE_INFINITY))).toBe(true);
  });

  it("keeps every default within the allowed range", () => {
    for (const value of Object.values(DEFAULT_LAYOUT)) {
      expect(clampFraction(value)).toBe(value);
    }
  });
});

// Shift+F11 on Diff, then Ctrl+3: the tick of Graph read unticked, the click hid it
// unseen, and Shift+F11 came back to a window without a graph.
describe("View ▸ <Panel> while another panel is maximised", () => {
  it("shows the panel it names, as its unticked box promised", async () => {
    const { layout } = await import("./layout.svelte");
    layout.reset();
    layout.toggleMaximized("diff");
    expect(layout.visible("graph")).toBe(false);

    layout.togglePanel("graph");

    expect(layout.maximized).toBeNull();
    expect(layout.visible("graph")).toBe(true);
    expect(layout.visible("diff")).toBe(true);
  });

  it("hides the maximised panel it names and brings the others back", async () => {
    const { layout } = await import("./layout.svelte");
    layout.reset();
    layout.toggleMaximized("diff");

    layout.togglePanel("diff");

    expect(layout.maximized).toBeNull();
    expect(layout.visible("diff")).toBe(false);
    expect(layout.visible("graph")).toBe(true);
  });

  it("still toggles a panel when none is maximised", async () => {
    const { layout } = await import("./layout.svelte");
    layout.reset();

    layout.togglePanel("graph");
    expect(layout.visible("graph")).toBe(false);
    layout.togglePanel("graph");
    expect(layout.visible("graph")).toBe(true);
  });
});
