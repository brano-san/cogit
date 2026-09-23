import { describe, expect, it } from "vitest";
import type { TodoEntry } from "./ipc";
import {
  ROOT_BASE,
  authorProblem,
  baseBefore,
  fullMessage,
  modifyPlan,
  rewordPlan,
  squashPlan,
} from "./rewrite-plans";

const plan: TodoEntry[] = [
  { oid: "a", action: "pick", message: "first" },
  { oid: "b", action: "pick", message: "second" },
  { oid: "c", action: "pick", message: "third" },
];

describe("baseBefore", () => {
  it("starts at the parent, or at the root for the first commit", () => {
    expect(baseBefore("b", ["a"])).toBe("b^");
    expect(baseBefore("a", [])).toBe(ROOT_BASE);
  });
});

describe("modifyPlan", () => {
  it("stops at the commit and picks the rest", () => {
    expect(modifyPlan(plan, "b")?.map((entry) => entry.action)).toEqual(["pick", "edit", "pick"]);
  });

  it("refuses a commit the plan does not hold", () => {
    expect(modifyPlan(plan, "z")).toBeNull();
  });
});

describe("rewordPlan", () => {
  it("rewords only that commit, with the whole new message", () => {
    const next = rewordPlan(plan, "c", "Third\n\nbody");
    expect(next?.[2]).toEqual({ oid: "c", action: "reword", message: "Third\n\nbody" });
    expect(next?.[0]).toEqual(plan[0]);
  });
});

describe("squashPlan", () => {
  it("squashes the commit into the one before it and lets Git join the messages", () => {
    const next = squashPlan(plan, "b");
    expect(next?.map((entry) => entry.action)).toEqual(["pick", "squash", "pick"]);
    expect(next?.[1]?.message).toBeNull();
  });

  it("cannot squash the first entry, which has nothing before it in the plan", () => {
    expect(squashPlan(plan, "a")).toBeNull();
  });
});

describe("authorProblem", () => {
  it("needs a name and an email without angle brackets", () => {
    expect(authorProblem(" ", "a@b")).toMatch(/name/);
    expect(authorProblem("A", "")).toMatch(/email/);
    expect(authorProblem("A <x>", "a@b")).toMatch(/Angle/);
    expect(authorProblem("A", "a@b")).toBeNull();
  });
});

describe("fullMessage", () => {
  it("joins the subject and the body with a blank line", () => {
    expect(fullMessage({ summary: "Subject", body: "Body" })).toBe("Subject\n\nBody");
    expect(fullMessage({ summary: "Subject", body: "" })).toBe("Subject");
  });
});
