import { describe, expect, it, vi } from "vitest";
import { revealRef } from "./ref-reveal";

const deps = (walk: Set<string>) => ({
  inWalk: vi.fn(async (oid: string) => walk.has(oid)),
  reveal: vi.fn(),
});

// A click on a row ticked the branch when its tip was off the graph; only the box may.
describe("a click on a ref's name", () => {
  it("centres on a tip the graph already has", async () => {
    const all = deps(new Set(["tip"]));
    await revealRef("tip", all);
    expect(all.reveal).toHaveBeenCalledWith("tip");
  });

  it("leaves a tip the walk does not reach alone, and no request behind", async () => {
    const all = deps(new Set());
    await revealRef("tip", all);
    expect(all.reveal).not.toHaveBeenCalled();
  });
});
