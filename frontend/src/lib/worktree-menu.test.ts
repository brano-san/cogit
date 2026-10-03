import { describe, expect, it } from "vitest";
import type { ContextItem, WorktreeEntry } from "./ipc";
import {
  moveTarget,
  moveTargetProblem,
  parseWorktreeCommand,
  pruneBlocked,
  worktreeHeaderMenu,
  worktreeMenu,
} from "./worktree-menu";

const LINKED: WorktreeEntry = {
  path: "D:/src/wt",
  name: "wt",
  branch: "topic",
  head: "abc",
  isMain: false,
  isCurrent: false,
  locked: null,
  missing: false,
  dirty: false,
  changed: 0,
  untracked: 0,
  bare: false,
  hasSubmodules: false,
};

const menu = (entry: WorktreeEntry) => worktreeMenu(entry, "Explorer");
const find = (items: ContextItem[], id: string) => items.find((item) => item.id === id);
const labels = (items: ContextItem[]) => items.map((item) => (item.separator ? "—" : item.label));

// `popup_context_menu` returns nothing: the choice arrives later as a menu command. Its
// bare `open` was the palette's Open Repository…, and every other item did nothing.
describe("the menu of a Worktrees row", () => {
  it("names every item so that it comes back as a worktree command", () => {
    for (const entry of [LINKED, { ...LINKED, missing: true }, { ...LINKED, locked: "" }]) {
      for (const item of menu(entry)) {
        if (item.separator) continue;
        expect(parseWorktreeCommand(item.id)).not.toBeNull();
      }
    }
  });

  it("never answers for an id that is not its own", () => {
    for (const id of ["open", "copy", "worktree-add", "worktree-remove", "worktree-prune", "repo-open", "wt-stage"]) {
      expect(parseWorktreeCommand(id)).toBeNull();
    }
  });

  it("offers the open, folder, terminal, lock, move and remove items for a linked worktree", () => {
    expect(labels(menu(LINKED))).toEqual([
      "Open",
      "Open in Explorer",
      "Open in Terminal",
      "Copy Path",
      "—",
      "Lock…",
      "Move…",
      "Remove…",
    ]);
    expect(parseWorktreeCommand(find(menu(LINKED), "worktree-row-open")!.id)).toBe("open");
    expect(find(menu(LINKED), "worktree-row-open")?.accelerator).toBe("Enter");
    expect(find(menu(LINKED), "worktree-row-remove")?.accelerator).toBe("Delete");
  });

  it("names the platform's file manager", () => {
    expect(find(worktreeMenu(LINKED, "Finder"), "worktree-row-folder")?.label).toBe("Open in Finder");
  });

  // A locked worktree goes with --force (git's two); it cannot move until unlocked.
  it("offers Unlock instead of Lock once it is locked, and still Remove", () => {
    const items = menu({ ...LINKED, locked: "on a USB disk" });
    expect(find(items, "worktree-row-unlock")?.enabled).toBe(true);
    expect(find(items, "worktree-row-remove")?.enabled).toBe(true);
    expect(find(items, "worktree-row-move")?.enabled).toBe(false);
    expect(find(items, "worktree-row-lock")).toBeUndefined();
  });

  it("turns off Remove, Move and Lock on the main worktree, and says why", () => {
    const items = menu({ ...LINKED, isMain: true });
    for (const off of ["worktree-row-remove", "worktree-row-move", "worktree-row-lock"]) {
      expect(find(items, off)?.enabled).toBe(false);
      expect(find(items, off)?.label).toContain("main worktree");
    }
  });

  it("turns off Open, Remove and Move on the one open in Cogit", () => {
    const items = menu({ ...LINKED, isCurrent: true });
    for (const off of ["worktree-row-open", "worktree-row-remove", "worktree-row-move"]) {
      expect(find(items, off)?.enabled).toBe(false);
    }
  });
});

describe("the Move… field", () => {
  it("takes a new folder with slashes as git lists them", () => {
    expect(moveTarget("  D:\\src\\moved\\ ")).toBe("D:/src/moved");
    expect(moveTargetProblem("D:\\src\\moved", "D:/src/wt")).toBeNull();
  });

  it("refuses nothing and the folder it is in now", () => {
    expect(moveTargetProblem("  ", "D:/src/wt")).not.toBeNull();
    expect(moveTargetProblem("d:\\SRC\\wt\\", "D:/src/wt")).not.toBeNull();
  });
});

// `git worktree remove --force` refuses a locked worktree, and a missing one had no
// Unlock anywhere: it could not be got rid of from Cogit at all.
describe("a missing worktree that is locked", () => {
  const gone = { ...LINKED, missing: true, locked: "" };

  it("can be unlocked from its menu", () => {
    expect(find(menu(gone), "worktree-row-unlock")?.enabled).toBe(true);
  });

  it("cannot be pruned until it is, and says why", () => {
    const prune = find(menu(gone), "worktree-row-prune");
    expect(prune?.enabled).toBe(false);
    expect(prune?.label).toContain("unlock");
    expect(pruneBlocked(gone)).not.toBeNull();
    expect(pruneBlocked({ ...gone, locked: null })).toBeNull();
  });

  it("offers no Unlock while it is not locked", () => {
    expect(find(menu({ ...LINKED, missing: true }), "worktree-row-unlock")).toBeUndefined();
    expect(find(menu({ ...LINKED, missing: true }), "worktree-row-prune")?.enabled).toBe(true);
  });
});

// A narrow panel clipped its header buttons; they fold into one menu that runs the
// palette's own commands.
describe("the menu of a narrow Worktrees header", () => {
  it("holds Add and Prune All as the palette's commands", () => {
    const items = worktreeHeaderMenu([LINKED]);
    expect(items.map((item) => item.id)).toEqual(["worktree-add", "worktree-prune"]);
    expect(items.every((item) => parseWorktreeCommand(item.id) === null)).toBe(true);
  });

  it("turns Prune All off while nothing is missing, and counts what it forgets", () => {
    expect(find(worktreeHeaderMenu([LINKED]), "worktree-prune")?.enabled).toBe(false);
    const on = find(worktreeHeaderMenu([{ ...LINKED, missing: true }]), "worktree-prune");
    expect(on?.enabled).toBe(true);
    expect(on?.label).toBe("Prune All (1)…");
  });
});
