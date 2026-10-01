import { expect, it, vi } from "vitest";
import type { RepoId, Submodule } from "$lib/ipc";

const mod = (path: string): Submodule => ({
  name: path,
  path,
  url: "",
  recorded: "",
  checkedOut: null,
  state: "unread",
  branch: null,
  subject: null,
  nested: false,
  ahead: 0,
  behind: 0,
  repoState: null, update: null, resolvedUrl: null,
});

let release: () => void = () => {};
vi.mock("$lib/ipc", () => ({
  listSubmodules: vi.fn(() => new Promise<Submodule[]>((done) => (release = () => done([mod("lib")])))),
  updateSubmodule: vi.fn(),
}));
vi.mock("$lib/ipc/repo-rows", () => ({ submoduleOutline: vi.fn(async () => [mod("lib")]) }));
vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });

const { submodules } = await import("./submodules.svelte");
const { moduleForest } = await import("./module-forest.svelte");
const { moduleMemory } = await import("./module-memory.svelte");

it("keeps the outline on screen while the owned tree is read", async () => {
  moduleMemory.setOpen("/a", true);
  await moduleForest.probe(["/a"]);
  const owning = submodules.own("r" as unknown as RepoId, "/a");
  expect(submodules.rows.map((row) => row.key)).toEqual(["lib"]);
  release();
  await owning;
  expect(submodules.rows.map((row) => row.key)).toEqual(["lib"]);
});

it("keeps a repository's read labels for its list row and for coming back", async () => {
  const { listSubmodules } = await import("$lib/ipc");
  const labeled = { ...mod("lib"), state: "inSync" as const, branch: "main" };
  vi.mocked(listSubmodules).mockImplementation(async () => [labeled]);
  await submodules.own("a" as unknown as RepoId, "/keep-a");
  await submodules.own("b" as unknown as RepoId, "/keep-b");
  expect(moduleForest.trees.get("/keep-a")?.get("")?.[0]?.branch).toBe("main");
  // Returning while the read fails must not blank the labels.
  vi.mocked(listSubmodules).mockImplementation(async () => Promise.reject(new Error("busy")));
  await submodules.own("a" as unknown as RepoId, "/keep-a");
  expect(submodules.top[0]?.branch).toBe("main");
});
