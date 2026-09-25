import { describe, expect, it, vi } from "vitest";
import { revealRef, type RevealDeps } from "./ref-reveal";

function deps(walk: Set<string>, over: Partial<RevealDeps> = {}) {
  const order: string[] = [];
  const all: RevealDeps = {
    inWalk: vi.fn(async (oid: string) => walk.has(oid)),
    tickable: true,
    tick: vi.fn(async () => {
      order.push("tick");
      walk.add("tip");
    }),
    reveal: vi.fn((oid: string) => void order.push(`reveal ${oid}`)),
    ...over,
  };
  return { all, order };
}

// F-128: with only HEAD ticked, a branch off HEAD's history was not in the walk, so the
// graph never scrolled to it, and the request hung until some later reload jumped there.
describe("a click on a ref's name", () => {
  it("centres on a tip the graph already has, ticking nothing", async () => {
    const { all, order } = deps(new Set(["tip"]));

    await revealRef("tip", all);

    expect(order).toEqual(["reveal tip"]);
  });

  it("ticks a ref whose tip the walk does not reach, then centres on it", async () => {
    const { all, order } = deps(new Set());

    await revealRef("tip", all);

    expect(order).toEqual(["tick", "reveal tip"]);
  });

  it("leaves no request behind for a tip that cannot be shown", async () => {
    const untickable = deps(new Set(), { tickable: false });
    await revealRef("tip", untickable.all);
    expect(untickable.order).toEqual([]);

    const filtered = deps(new Set(), { tick: vi.fn(async () => {}) });
    await revealRef("tip", filtered.all);
    expect(filtered.all.reveal).not.toHaveBeenCalled();
  });
});
