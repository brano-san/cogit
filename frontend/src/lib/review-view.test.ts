import { describe, expect, it } from "vitest";
import { baseOptions, reloadsOn } from "./review-view";

describe("the base picker", () => {
  it("offers every branch but the reviewed one and remote HEAD pointers", () => {
    expect(baseOptions(["main", "topic", "origin/HEAD", "origin/main"], "topic", "main")).toEqual([
      ["main", "main"],
      ["origin/main", "origin/main"],
    ]);
  });

  it("keeps a base the list does not have", () => {
    expect(baseOptions(["main"], "topic", "v1.0")[0]).toEqual(["v1.0", "v1.0"]);
  });

  it("asks for a base while there is none", () => {
    expect(baseOptions(["dev"], "topic", null)[0]).toEqual(["", "Choose a base"]);
  });
});

describe("reloading", () => {
  it("follows the refs always and the working tree only while it is included", () => {
    expect(reloadsOn("refs", false)).toBe(true);
    expect(reloadsOn("workingTree", false)).toBe(false);
    expect(reloadsOn("workingTree", true)).toBe(true);
    expect(reloadsOn("stash", true)).toBe(false);
  });
});
