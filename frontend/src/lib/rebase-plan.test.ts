import { describe, expect, it } from "vitest";
import { moveEntry, planProblem, previewCount } from "./rebase-plan";
import type { TodoEntry } from "./ipc";

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
