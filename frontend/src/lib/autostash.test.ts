import { describe, expect, it, vi } from "vitest";
import { switchWithAutostash, type AutostashSteps } from "./autostash";

function steps(over: Partial<AutostashSteps> = {}) {
  const order: string[] = [];
  const all: AutostashSteps = {
    ask: vi.fn(async () => true),
    run: vi.fn(async () => void order.push("run")),
    report: vi.fn((_err: unknown, title: string) => void order.push(`report: ${title}`)),
    ...over,
  };
  return { all, order };
}

// F-132: Switch branch with local changes in the way — stash, check out, put them back.
// The steps themselves, their order and what a refusal at each leaves behind are the
// backend's one operation now (git_engine tests/autostash.rs).
describe("switching with the changes stashed out of the way", () => {
  it("asks, naming what is in the way, then runs the switch as one step", async () => {
    const { all, order } = steps();

    expect(await switchWithAutostash("topic", ["a.txt"], all)).toBe("done");

    expect(order).toEqual(["run"]);
    expect(all.ask).toHaveBeenCalledWith(expect.stringContaining("a.txt"));
  });

  it("changes nothing when the user declines", async () => {
    const { all, order } = steps({ ask: vi.fn(async () => false) });

    expect(await switchWithAutostash("topic", [], all)).toBe("declined");

    expect(order).toEqual([]);
  });

  it("reports what the switch refused, and the working tree may have changed", async () => {
    const { all, order } = steps({
      run: vi.fn(async () => {
        throw new Error("checkout refused");
      }),
    });

    expect(await switchWithAutostash("topic", [], all)).toBe("done");

    expect(order).toEqual(["report: Could not switch branches"]);
  });
});
