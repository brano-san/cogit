import { describe, expect, it } from "vitest";
import { editedSteps, movedSteps, todoMenu } from "./rebase-todo";

const steps = [
  { action: "pick", oid: "a1", summary: "one" },
  { action: "p", oid: "b2", summary: "two" },
  { action: "pick", oid: "c3", summary: "three" },
];

describe("the menu of a step left in a rebase", () => {
  it("offers every verb but the one it has, and moves where there is room", () => {
    const menu = todoMenu(steps, 0);
    const enabled = menu.filter((item) => item.enabled).map((item) => item.id);
    expect(enabled).toEqual([
      "rebase-todo-edit",
      "rebase-todo-squash",
      "rebase-todo-fixup",
      "rebase-todo-drop",
      "rebase-todo-down",
    ]);
    expect(todoMenu(steps, 1).find((item) => item.id === "rebase-todo-pick")?.enabled).toBe(false);
    expect(todoMenu(steps, 2).find((item) => item.id === "rebase-todo-down")?.enabled).toBe(false);
  });
});

describe("the steps after a menu item", () => {
  it("changes the verb of one step and spells the short ones out", () => {
    expect(editedSteps(steps, 2, "rebase-todo-squash")?.map((step) => step.action)).toEqual(["pick", "pick", "squash"]);
  });

  it("moves a step up or down", () => {
    expect(editedSteps(steps, 1, "rebase-todo-up")?.map((step) => step.oid)).toEqual(["b2", "a1", "c3"]);
    expect(editedSteps(steps, 1, "rebase-todo-down")?.map((step) => step.oid)).toEqual(["a1", "c3", "b2"]);
    expect(editedSteps(steps, 0, "rebase-todo-up")).toBeNull();
  });

  it("is nothing for another menu's item", () => {
    expect(editedSteps(steps, 0, "ref:merge")).toBeNull();
  });
});

describe("a step dragged onto another", () => {
  const steps = [
    { action: "p", oid: "a", summary: "A" },
    { action: "pick", oid: "b", summary: "B" },
    { action: "squash", oid: "c", summary: "C" },
  ];

  it("takes the place of the step it lands on, verbs spelled out", () => {
    expect(movedSteps(steps, 0, 2)?.map((step) => step.oid)).toEqual(["b", "c", "a"]);
    expect(movedSteps(steps, 2, 0)?.map((step) => step.oid)).toEqual(["c", "a", "b"]);
    expect(movedSteps(steps, 0, 2)?.[2]?.action).toBe("pick");
  });

  it("changes nothing on itself or off the list", () => {
    expect(movedSteps(steps, 1, 1)).toBeNull();
    expect(movedSteps(steps, 1, 5)).toBeNull();
  });
});
