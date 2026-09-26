import { describe, expect, it } from "vitest";
import { EMPTY_MEMORY, forgetRoot, readModuleMemory, withNode, withOpenedRepository, withTop } from "./module-memory";

describe("module memory", () => {
  it("keeps several repositories open at once", () => {
    const memory = withTop(withTop(EMPTY_MEMORY, "/a", true), "/b", true);
    expect(memory.open).toEqual(["/a", "/b"]);
    expect(withTop(memory, "/a", false).open).toEqual(["/b"]);
  });

  it("keeps expanded nodes per repository and drops an emptied one", () => {
    let memory = withNode(EMPTY_MEMORY, "/a", "vendor/lib", true);
    memory = withNode(memory, "/b", "x", true);
    expect(memory.nodes).toEqual({ "/a": ["vendor/lib"], "/b": ["x"] });
    expect(withNode(memory, "/a", "vendor/lib", false).nodes).toEqual({ "/b": ["x"] });
  });

  it("returns the same object when nothing changes, so the store writes nothing", () => {
    const memory = withTop(EMPTY_MEMORY, "/a", true);
    expect(withTop(memory, "/a", true)).toBe(memory);
    expect(withNode(memory, "/a", "k", false)).toBe(memory);
    expect(forgetRoot(memory, "/elsewhere")).toBe(memory);
  });

  it("survives a round trip through storage and ignores what it cannot use", () => {
    const memory = withNode(withTop(EMPTY_MEMORY, "/a", true), "/a", "k", true);
    expect(readModuleMemory(JSON.parse(JSON.stringify(memory)))).toEqual(memory);
    expect(readModuleMemory({ open: ["/a", 3, "/a"], nodes: { "/b": "x", "/c": ["k"] } })).toEqual({
      open: ["/a"],
      nodes: { "/c": ["k"] },
    });
    expect(readModuleMemory("garbage")).toEqual(EMPTY_MEMORY);
  });

  it("forgets a removed repository entirely", () => {
    const memory = withNode(withTop(EMPTY_MEMORY, "/a", true), "/a", "k", true);
    expect(forgetRoot(memory, "/a")).toEqual(EMPTY_MEMORY);
  });
});

// Item 42 of 25.09: a repository just opened showed its submodules only after a click on
// its triangle.
describe("a repository just opened", () => {
  it("shows its submodules, one level of them", () => {
    const memory = withOpenedRepository(EMPTY_MEMORY, "/a");
    expect(memory.open).toEqual(["/a"]);
    expect(memory.nodes).toEqual({});
  });

  it("leaves the nodes below as they were remembered", () => {
    const before = withNode(EMPTY_MEMORY, "/a", "vendor/lib", false);
    const opened = withOpenedRepository(withNode(before, "/b", "x", true), "/a");
    expect(opened.nodes).toEqual({ "/b": ["x"] });
    expect(withOpenedRepository(opened, "/a")).toBe(opened);
  });
});
