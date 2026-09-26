import { describe, expect, it, vi } from "vitest";
import { droppedRepositories, openDropped, type DropHost } from "./drop-open";

function host(refuse: string[] = []) {
  const steps: string[] = [];
  const fake: DropHost = {
    openInList: vi.fn(async (path: string) => {
      steps.push(`list ${path}`);
      if (refuse.includes(path)) throw new Error(`${path} is not a repository`);
    }),
    activate: vi.fn(async (path: string) => {
      steps.push(`show ${path}`);
    }),
    refreshList: vi.fn(async () => {
      steps.push("refresh");
    }),
    report: vi.fn(),
  };
  return { fake, steps };
}

// Three folders dropped at once took the panels one after another: Branches and the headings
// showed the second and the third over the first one's graph, then the first.
describe("openDropped", () => {
  it("lists every folder but the first without the panels, then shows the first", async () => {
    const { fake, steps } = host();

    await openDropped(["C:/a", "C:/b", "C:/c"], fake);

    expect(steps).toEqual(["list C:/b", "list C:/c", "refresh", "show C:/a"]);
  });

  it("reports a folder that does not open and goes on with the rest", async () => {
    const { fake, steps } = host(["C:/b"]);

    await openDropped(["C:/a", "C:/b", "C:/c"], fake);

    expect(fake.report).toHaveBeenCalledWith(expect.any(Error), "Could not open the repository");
    expect(steps).toContain("list C:/c");
    expect(steps.at(-1)).toBe("show C:/a");
  });
});

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
