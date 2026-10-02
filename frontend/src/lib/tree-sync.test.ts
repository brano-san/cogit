import { describe, expect, it, vi } from "vitest";
import type { TreeChanged } from "$lib/ipc";
import type { Listen } from "./settings-sync";
import { announceTreeChange, onTreeChange } from "./tree-sync";

describe("tree-sync", () => {
  it("announces the repository and the file", async () => {
    const emit = vi.fn(async (_: TreeChanged) => {});
    await announceTreeChange({ repo: 1, path: "a.txt" }, emit);
    expect(emit).toHaveBeenCalledWith({ repo: 1, path: "a.txt" });
  });

  it("hands the change to the handler until undone", async () => {
    let send: (event: { payload: TreeChanged }) => void = () => {};
    const listen: Listen<TreeChanged> = async (h) => {
      send = h;
      return () => {};
    };
    const handler = vi.fn();
    const undo = onTreeChange(handler, listen);
    await Promise.resolve();
    send({ payload: { repo: 2, path: "b" } });
    undo();
    send({ payload: { repo: 2, path: "c" } });
    expect(handler).toHaveBeenCalledExactlyOnceWith({ repo: 2, path: "b" });
  });
});
