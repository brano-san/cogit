import { describe, expect, it, vi } from "vitest";
import { switchWithAutostash, type AutostashSteps } from "./autostash";

function steps(over: Partial<AutostashSteps> = {}) {
  const order: string[] = [];
  const step = (name: string) => vi.fn(async () => void order.push(name));
  const all: AutostashSteps = {
    ask: vi.fn(async () => true),
    stash: step("stash"),
    checkout: step("checkout"),
    pop: step("pop"),
    report: vi.fn((_err: unknown, title: string) => void order.push(`report: ${title}`)),
    ...over,
  };
  return { all, order };
}

const refused = (name: string) =>
  vi.fn(async () => {
    throw new Error(`${name} refused`);
  });

// F-132: Switch branch with local changes in the way — stash, check out, put them back.
describe("switching with the changes stashed out of the way", () => {
  it("stashes, checks out, then puts the changes back, in that order", async () => {
    const { all, order } = steps();

    expect(await switchWithAutostash("topic", ["a.txt"], all)).toBe("done");

    expect(order).toEqual(["stash", "checkout", "pop"]);
    expect(all.ask).toHaveBeenCalledWith(expect.stringContaining("a.txt"));
  });

  it("changes nothing when the user declines", async () => {
    const { all, order } = steps({ ask: vi.fn(async () => false) });

    expect(await switchWithAutostash("topic", [], all)).toBe("declined");

    expect(order).toEqual([]);
  });

  it("puts the changes back when the checkout is refused", async () => {
    const { all, order } = steps();
    all.checkout = vi.fn(async () => {
      order.push("checkout");
      throw new Error("checkout refused");
    });

    expect(await switchWithAutostash("topic", [], all)).toBe("done");

    expect(order).toEqual(["stash", "checkout", "pop", "report: Could not switch branches"]);
  });

  it("says where the changes are when they cannot be put back after a refused checkout", async () => {
    const { all, order } = steps({ checkout: refused("checkout"), pop: refused("pop") });

    await switchWithAutostash("topic", [], all);

    expect(order).toEqual([
      "stash",
      "report: Your changes are in stash@{0}: they could not be put back",
      "report: Could not switch branches",
    ]);
  });

  it("goes no further when the stash itself fails", async () => {
    const { all, order } = steps({ stash: refused("stash") });

    expect(await switchWithAutostash("topic", [], all)).toBe("done");

    expect(order).toEqual(["report: Could not stash the changes"]);
    expect(all.checkout).not.toHaveBeenCalled();
  });

  it("reports a pop that conflicts after the switch, which is where it stays", async () => {
    const { all, order } = steps({ pop: refused("pop") });

    await switchWithAutostash("topic", [], all);

    expect(order).toEqual(["stash", "checkout", "report: Could not put the changes back"]);
  });
});
