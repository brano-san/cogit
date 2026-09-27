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
