import { describe, expect, it } from "vitest";
import { MENU_PREFIX, actionOf, solverMenu } from "./solver-menu";

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
    ]);
    expect(take.map((entry) => entry.accelerator)).toEqual(["CmdOrCtrl+1", "CmdOrCtrl+2", "CmdOrCtrl+3", "CmdOrCtrl+4"]);
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
