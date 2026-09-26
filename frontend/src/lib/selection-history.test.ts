import { describe, expect, it } from "vitest";
import { SelectionHistory, homeTarget } from "./selection-history";

describe("homeTarget", () => {
  it("goes from the Working Tree to HEAD's commit", () => {
    expect(homeTarget(null, "head")).toBe("head");
  });

  it("goes from HEAD's commit to the Working Tree", () => {
    expect(homeTarget("head", "head")).toBeNull();
  });

  it("goes from any other commit to the Working Tree", () => {
    expect(homeTarget("other", "head")).toBeNull();
  });

  it("stays on the Working Tree when there is no commit yet", () => {
    expect(homeTarget(null, null)).toBeNull();
  });
});

describe("SelectionHistory", () => {
  it("has nothing to go back to at first", () => {
    const history = new SelectionHistory();
    history.visit(null);
    expect(history.canGoBack).toBe(false);
    expect(history.back()).toBeUndefined();
  });

  it("goes back to the selections before, newest first", () => {
    const history = new SelectionHistory();
    history.visit(null);
    history.visit("a");
    history.visit("b");

    expect(history.back()).toBe("a");
    expect(history.back()).toBeNull();
    expect(history.canGoBack).toBe(false);
  });

  it("does not record going back as a new step", () => {
    const history = new SelectionHistory();
    history.visit("a");
    history.visit("b");
    const target = history.back();
    history.visit(target ?? null);

    expect(history.canGoBack).toBe(false);
  });

  it("records the same selection twice in a row once", () => {
    const history = new SelectionHistory();
    history.visit("a");
    history.visit("a");
    history.visit("b");
    history.visit("b");

    expect(history.back()).toBe("a");
    expect(history.canGoBack).toBe(false);
  });

  it("goes on from wherever Back left it", () => {
    const history = new SelectionHistory();
    history.visit("a");
    history.visit("b");
    history.visit("c");
    history.back();
    history.visit("d");

    expect(history.back()).toBe("b");
    expect(history.back()).toBe("a");
  });

  it("keeps a bounded number of steps", () => {
    const history = new SelectionHistory();
    for (let i = 0; i <= SelectionHistory.LIMIT + 10; i++) history.visit(`c${i}`);
    let steps = 0;
    while (history.back() !== undefined) steps++;
    expect(steps).toBe(SelectionHistory.LIMIT);
  });
});
