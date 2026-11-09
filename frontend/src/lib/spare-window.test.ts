import { describe, expect, it } from "vitest";
import { windowOf } from "./spare-window";

describe("windowOf", () => {
  it("names the window a child address opens", () => {
    expect(windowOf("/commit.html?repo=1&root=x")).toBe("commit");
    expect(windowOf("/blame.html")).toBe("blame");
  });

  it("knows no window for any other page", () => {
    expect(windowOf("/index.html")).toBeNull();
    expect(windowOf("/spare.html")).toBeNull();
  });
});
