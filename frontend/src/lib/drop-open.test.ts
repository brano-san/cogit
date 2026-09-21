import { describe, expect, it } from "vitest";
import { droppedRepositories } from "./drop-open";

describe("droppedRepositories", () => {
  it("keeps the paths as they came", () => {
    expect(droppedRepositories(["C:/a", "C:/b"])).toEqual(["C:/a", "C:/b"]);
  });

  it("opens one folder once even if the drop repeats it", () => {
    expect(droppedRepositories(["C:/a", "C:/a"])).toEqual(["C:/a"]);
  });

  it("treats a trailing separator as the same folder", () => {
    expect(droppedRepositories(["C:/a/", "C:/a"])).toEqual(["C:/a"]);
  });

  it("strips a trailing backslash too, which is what Windows drops", () => {
    expect(droppedRepositories(["C:\\work\\repo\\"])).toEqual(["C:\\work\\repo"]);
  });

  it("drops a blank path rather than asking the backend about it", () => {
    expect(droppedRepositories(["", "   ", "C:/a"])).toEqual(["C:/a"]);
  });

  it("caps the drop so a folder of a hundred repositories cannot flood the app", () => {
    const many = Array.from({ length: 30 }, (_, n) => `C:/r${n}`);
    expect(droppedRepositories(many)).toHaveLength(8);
  });

  it("takes the first ones, which is the order they were dropped in", () => {
    const many = Array.from({ length: 30 }, (_, n) => `C:/r${n}`);
    expect(droppedRepositories(many, 2)).toEqual(["C:/r0", "C:/r1"]);
  });

  it("returns nothing for an empty drop", () => {
    expect(droppedRepositories([])).toEqual([]);
  });
});
