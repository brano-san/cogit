import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { Submodule } from "$lib/ipc";

const mod = (path: string, nested = false): Submodule => ({
  name: path,
  path,
  url: "",
  recorded: "",
  checkedOut: null,
  state: "unread",
  branch: null,
  subject: null,
  nested,
  ahead: 0,
  behind: 0,
  repoState: null,
});

/** What each folder's `.gitmodules` says, by root and key from the top. */
const OUTLINES: Record<string, Submodule[]> = {
  "/a:": [mod("vendor/lib", true)],
  "/a:vendor/lib": [mod("deep")],
  "/b:": [mod("ext")],
  "/c:": [],
};

let inFlight = 0;
let widest = 0;
const outline = vi.fn(async (root: string, parent = "") => {
  inFlight += 1;
  widest = Math.max(widest, inFlight);
  await Promise.resolve();
  inFlight -= 1;
  return OUTLINES[`${root}:${parent}`] ?? [];
});
vi.mock("$lib/ipc/repo-rows", () => ({ submoduleOutline: outline }));

const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, value: string) => void store.set(key, value),
  removeItem: (key: string) => void store.delete(key),
});

async function fresh() {
  vi.resetModules();
  return (await import("./module-forest.svelte")).moduleForest;
}

describe("the submodule trees of repositories not on screen", () => {
  // The first import compiles the store and all it pulls in; on a busy machine that alone
  // outlasts a test's timeout, and the cut-off test then runs on into the next one.
  beforeAll(() => import("./module-forest.svelte"), 60_000);

  beforeEach(() => {
    store.clear();
    outline.mockClear();
    widest = 0;
  });

  it("knows which rows have a tree before any is opened", async () => {
    const forest = await fresh();
    await forest.probe(["/a", "/b", "/c"]);
    expect(forest.hasModules("/a")).toBe(true);
    expect(forest.hasModules("/c")).toBe(false);
    expect(forest.hasModules("/elsewhere")).toBeUndefined();
    expect(forest.rows("/a")).toEqual([]);
  });

  it("reads one repository at a time and each only once", async () => {
    const forest = await fresh();
    await Promise.all([forest.probe(["/a", "/b", "/c"]), forest.probe(["/a", "/b"])]);
    expect(widest).toBe(1);
    expect(outline).toHaveBeenCalledTimes(3);
  });

  it("keeps two projects open at once", async () => {
    const forest = await fresh();
    await forest.toggleTop("/a");
    await forest.toggleTop("/b");
    expect(forest.rows("/a").map((row) => row.key)).toEqual(["vendor/lib"]);
    expect(forest.rows("/b").map((row) => row.key)).toEqual(["ext"]);
  });

  it("opens a nested node by reading only that node", async () => {
    const forest = await fresh();
    await forest.toggleTop("/a");
    outline.mockClear();
    await forest.toggle("/a", forest.rows("/a")[0]!);
    expect(outline).toHaveBeenCalledWith("/a", "vendor/lib");
    expect(forest.rows("/a").map((row) => [row.key, row.depth])).toEqual([
      ["vendor/lib", 0],
      ["vendor/lib/deep", 1],
    ]);
  });

  it("finds the trees as they were left after a restart", async () => {
    const before = await fresh();
    await before.toggleTop("/a");
    await before.toggle("/a", before.rows("/a")[0]!);
    await before.toggleTop("/b");
    await before.toggleTop("/b");

    const after = await fresh();
    await after.probe(["/a", "/b"]);
    expect(after.rows("/a").map((row) => row.key)).toEqual(["vendor/lib", "vendor/lib/deep"]);
    expect(after.rows("/b")).toEqual([]);
  });
});
