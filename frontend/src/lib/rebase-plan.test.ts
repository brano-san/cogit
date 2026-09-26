import { describe, expect, it, vi } from "vitest";
import { moveEntry, planChanged, planProblem, planPublished, previewCount } from "./rebase-plan";
import type { TodoEntry } from "./ipc";

// Two local commits on top of origin/main: the editor warned about a force-push because it
// asked about the base, which the rebase leaves alone.
describe("planPublished", () => {
  const plan = [
    { oid: "a", action: "pick", message: "a" },
    { oid: "b", action: "pick", message: "b" },
  ] as const satisfies readonly TodoEntry[];

  it("is false when none of the commits it rewrites is on a remote", async () => {
    const check = vi.fn(async (oid: string) => oid === "base");
    expect(await planPublished(plan, check)).toBe(false);
    expect(check).not.toHaveBeenCalledWith("base");
  });

  it("is true once one of them is, oldest first", async () => {
    const check = vi.fn(async (oid: string) => oid === "a");
    expect(await planPublished(plan, check)).toBe(true);
    expect(check).toHaveBeenCalledTimes(1);
  });

  it("finds a published commit a merge brought in later in the plan", async () => {
    expect(await planPublished(plan, async (oid) => oid === "b")).toBe(true);
  });
});

const pick = (oid: string): TodoEntry => ({ oid, action: "pick", message: oid });

describe("moveEntry", () => {
  const plan = [pick("a"), pick("b"), pick("c")];

  it("moves an entry down", () => {
    expect(moveEntry(plan, 0, 1).map((e) => e.oid)).toEqual(["b", "a", "c"]);
  });

  it("moves an entry up", () => {
    expect(moveEntry(plan, 2, 0).map((e) => e.oid)).toEqual(["c", "a", "b"]);
  });

  it("leaves the plan alone for a move to the same place", () => {
    expect(moveEntry(plan, 1, 1).map((e) => e.oid)).toEqual(["a", "b", "c"]);
  });

  it("ignores an index outside the plan", () => {
    expect(moveEntry(plan, 9, 0).map((e) => e.oid)).toEqual(["a", "b", "c"]);
    expect(moveEntry(plan, 0, -3).map((e) => e.oid)).toEqual(["a", "b", "c"]);
  });

  it("does not mutate the plan it was given", () => {
    moveEntry(plan, 0, 2);
    expect(plan.map((e) => e.oid)).toEqual(["a", "b", "c"]);
  });
});

describe("planProblem", () => {
  it("accepts a plan that starts with a pick", () => {
    expect(planProblem([pick("a"), { ...pick("b"), action: "squash" }])).toBeNull();
  });

  it("refuses a squash with nothing before it to squash into", () => {
    expect(planProblem([{ ...pick("a"), action: "squash" }])).toMatch(/nothing before it/i);
  });

  it("refuses a fixup as the first entry too", () => {
    expect(planProblem([{ ...pick("a"), action: "fixup" }])).toMatch(/nothing before it/i);
  });

  it("refuses a plan where everything is dropped", () => {
    expect(planProblem([{ ...pick("a"), action: "drop" }])).toMatch(/at least one/i);
  });

  it("refuses an empty plan", () => {
    expect(planProblem([])).toMatch(/nothing/i);
  });

  it("allows a squash after a dropped commit only if a pick came earlier", () => {
    const plan: TodoEntry[] = [
      pick("a"),
      { ...pick("b"), action: "drop" },
      { ...pick("c"), action: "squash" },
    ];
    expect(planProblem(plan)).toBeNull();
  });
});

describe("previewCount", () => {
  it("counts what the history will hold afterwards", () => {
    const plan: TodoEntry[] = [
      pick("a"),
      { ...pick("b"), action: "squash" },
      { ...pick("c"), action: "drop" },
      pick("d"),
    ];
    expect(previewCount(plan)).toBe(2);
  });
});

// A click beside Interactive Rebase threw away the reordered, reworded plan without a word.
describe("planChanged", () => {
  const plan: TodoEntry[] = [pick("a"), pick("b")];

  it("is false for the plan as it came", () => {
    expect(planChanged(plan, plan.map((entry) => ({ ...entry })))).toBe(false);
  });

  it("sees a move, a new action and a new message", () => {
    expect(planChanged(plan, moveEntry(plan, 0, 1))).toBe(true);
    expect(planChanged(plan, [{ ...pick("a"), action: "drop" }, pick("b")])).toBe(true);
    expect(planChanged(plan, [{ ...pick("a"), action: "reword", message: "better" }, pick("b")])).toBe(true);
  });
});
