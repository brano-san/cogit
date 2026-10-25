import { describe, expect, it } from "vitest";
import { BAR_MENUS, MENU_PREFIX, actionOf, solverBarMenu, solverMenu } from "./solver-menu";

const ALL = { hunk: true, changes: true, conflicts: true, tool: true };

const ids = (items: ReturnType<typeof solverMenu>) => items.filter((entry) => !entry.separator).map((entry) => entry.id);

describe("the menu of the Conflict Solver", () => {
  it("routes every row to this window, by its label", () => {
    const items = solverMenu("solver-3", ALL);
    expect(ids(items).every((id) => id.startsWith(`${MENU_PREFIX}solver-3:`))).toBe(true);
    expect(ids(items)).toContain("child:solver-3:solver-take-ours");
  });

  it("gives back the action a row stands for", () => {
    expect(actionOf("child:solver-3:solver-take-ours")).toBe("solver-take-ours");
  });

  it("offers every take action, with the keys that do the same", () => {
    const items = solverMenu("w", ALL);
    const take = items.filter((entry) => entry.id.includes("take"));
    expect(take.map((entry) => entry.label)).toEqual([
      "Take Ours",
      "Take Theirs",
      "Take Ours + Theirs",
      "Take Theirs + Ours",
      "Take Base",
    ]);
    expect(take.map((entry) => entry.accelerator)).toEqual([
      "CmdOrCtrl+1",
      "CmdOrCtrl+2",
      "CmdOrCtrl+3",
      "CmdOrCtrl+4",
      "CmdOrCtrl+5",
    ]);
  });

  it("keeps a row that does not apply, switched off", () => {
    const items = solverMenu("w", { hunk: false, changes: false, conflicts: false, tool: false });
    for (const entry of items.filter((each) => !each.separator)) expect(entry.enabled).toBe(false);
    expect(ids(items).length).toBe(ids(solverMenu("w", ALL)).length);
  });

  it("walking needs somewhere to walk to, taking needs a hunk", () => {
    const items = solverMenu("w", { hunk: false, changes: true, conflicts: false, tool: true });
    const on = (action: string) => items.find((entry) => entry.id.endsWith(action))?.enabled;
    expect(on("solver-take-ours")).toBe(false);
    expect(on("solver-next-change")).toBe(true);
    expect(on("solver-next-conflict")).toBe(false);
    expect(on("solver-external-tool")).toBe(true);
  });
});

describe("the menu bar of the Conflict Solver", () => {
  const STATE = {
    ...ALL,
    save: false,
    ready: true,
    deleted: { ours: true, theirs: false },
    layout: "all" as const,
    baseChanges: true,
    whole: false,
  };
  const rows = (menu: Parameters<typeof solverBarMenu>[1]) =>
    solverBarMenu("w", menu, STATE).filter((entry) => !entry.separator);

  it("has SmartGit's five menus", () => {
    expect(BAR_MENUS).toEqual(["File", "Edit", "View", "Go To", "Window"]);
  });

  it("offers the file's own actions, off where they cannot apply", () => {
    const file = rows("File");
    const on = (action: string) => file.find((entry) => entry.id.endsWith(action))?.enabled;
    expect(on("solver-save")).toBe(false);
    expect(on("solver-keep-ours")).toBe(false);
    expect(on("solver-keep-theirs")).toBe(true);
    expect(on("solver-delete-file")).toBe(true);
    expect(file.find((entry) => entry.id.endsWith("solver-save"))?.accelerator).toBe("CmdOrCtrl+S");
  });

  it("ticks the toggles and the layout shown", () => {
    const view = rows("View");
    const ticked = view.filter((entry) => entry.checked).map((entry) => actionOf(entry.id));
    expect(ticked).toEqual(["solver-base-changes", "solver-layout-all"]);
    expect(view.every((entry) => entry.checked !== undefined)).toBe(true);
  });

  it("goes to changes and conflicts, and takes in Edit", () => {
    expect(rows("Go To").map((entry) => actionOf(entry.id))).toEqual([
      "solver-prev-change",
      "solver-next-change",
      "solver-prev-conflict",
      "solver-next-conflict",
    ]);
    expect(rows("Edit").map((entry) => actionOf(entry.id))).toContain("solver-take-theirs-ours");
  });
});
