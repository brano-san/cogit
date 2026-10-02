import { describe, expect, it, vi } from "vitest";
import type { FileEntry, OpenModule, Submodule } from "$lib/ipc";
import { askToOpenModule, findModuleRow, isModulePath, onOpenModule } from "./module-open";
import type { Listen } from "./settings-sync";

const file = (path: string, mode: FileEntry["mode"] = "plain"): FileEntry => ({
  path,
  oldPath: null,
  status: "modified",
  mode,
  modeChange: null,
  similarity: null,
});

const module = (path: string): Submodule =>
  ({ name: path, path, url: "", recorded: "", checkedOut: null, state: "inSync" }) as unknown as Submodule;

describe("isModulePath", () => {
  // Double-clicking a changed submodule opened a compare window with nothing to compare.
  it("tells a submodule among the listed files", () => {
    const lists = [[file("a.txt")], [file("vendor/lib", "submodule")]];

    expect(isModulePath("vendor/lib", lists)).toBe(true);
    expect(isModulePath("a.txt", lists)).toBe(false);
    expect(isModulePath("gone.txt", lists)).toBe(false);
  });
});

describe("findModuleRow", () => {
  it("finds the submodule under the repository the panels show", async () => {
    const list = vi.fn();
    const children = new Map([["", [module("vendor/lib"), module("docs")]]]);

    const row = await findModuleRow(children, null, "vendor/lib", list);

    expect(row).toMatchObject({ key: "vendor/lib", path: "vendor/lib", parent: "", depth: 0 });
    expect(list).not.toHaveBeenCalled();
  });

  it("keys a nested one from the submodule on show, and reads its parent's list when needed", async () => {
    const list = vi.fn(async () => [module("deps/zlib")]);

    const row = await findModuleRow(new Map(), "vendor/lib", "deps/zlib", list);

    expect(list).toHaveBeenCalledWith("vendor/lib");
    expect(row).toMatchObject({ key: "vendor/lib/deps/zlib", path: "deps/zlib", parent: "vendor/lib", depth: 1 });
  });

  it("is null for a path that is no submodule of it", async () => {
    expect(await findModuleRow(new Map([["", [module("docs")]]]), null, "vendor/lib", vi.fn())).toBeNull();
  });
});

describe("askToOpenModule and onOpenModule", () => {
  it("carry the repository and the path from the compare window to the main one", async () => {
    let deliver: (event: { payload: OpenModule }) => void = () => {};
    const listen: Listen<OpenModule> = async (handler) => {
      deliver = handler;
      return () => {};
    };
    const seen: OpenModule[] = [];
    onOpenModule((request) => seen.push(request), listen);
    await Promise.resolve();

    await askToOpenModule({ repo: 3, path: "vendor/lib" }, async (payload) => deliver({ payload }));

    expect(seen).toEqual([{ repo: 3, path: "vendor/lib" }]);
  });

  it("stops listening when undone, even before the listener was in place", async () => {
    const unlisten = vi.fn();
    const handler = vi.fn();
    onOpenModule(handler, async () => unlisten)();
    await Promise.resolve();
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalledOnce();
  });
});
