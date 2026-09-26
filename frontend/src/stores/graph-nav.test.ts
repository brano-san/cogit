import { describe, expect, it } from "vitest";
import { graphNav } from "./graph-nav.svelte";

describe("graphNav", () => {
  it("goes back within the repository it was used in", () => {
    graphNav.track(1, null);
    graphNav.track(1, "a");
    graphNav.track(1, "b");
    expect(graphNav.canGoBack).toBe(true);

    expect(graphNav.back()).toBe("a");
    graphNav.track(1, "a");
    expect(graphNav.back()).toBeNull();
    expect(graphNav.canGoBack).toBe(false);
  });

  it("keeps each repository's history apart", () => {
    graphNav.track(2, null);
    graphNav.track(2, "x");
    graphNav.track(3, null);
    expect(graphNav.canGoBack).toBe(false);

    graphNav.track(2, "x");
    expect(graphNav.canGoBack).toBe(true);
    expect(graphNav.back()).toBeNull();
  });

  it("has nowhere to go without a repository", () => {
    graphNav.track(null, null);
    expect(graphNav.canGoBack).toBe(false);
    expect(graphNav.back()).toBeUndefined();
  });

  it("asks the list to go to its top", () => {
    const before = graphNav.top;
    graphNav.toTop();
    expect(graphNav.top).toBe(before + 1);
  });
});
