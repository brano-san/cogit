import { describe, expect, it, vi } from "vitest";
import { announceTreeChange, onTreeChange, TREE_CHANGED } from "./tree-sync";

describe("tree-sync", () => {
  it("announces the repository and the file", async () => {
    const emit = vi.fn(async () => {});
    await announceTreeChange({ repo: 1 as never, path: "a.txt" }, emit);
    expect(emit).toHaveBeenCalledWith(TREE_CHANGED, { repo: 1, path: "a.txt" });
  });

  it("hands valid changes to the handler and ignores junk", async () => {
    let send: (event: { payload: unknown }) => void = () => {};
    const handler = vi.fn();
    onTreeChange(handler, async (_name, h) => {
      send = h;
      return () => {};
    });
    await Promise.resolve();
    send({ payload: { repo: 2, path: "b" } });
    send({ payload: "nope" });
    expect(handler).toHaveBeenCalledOnce();
  });
});
