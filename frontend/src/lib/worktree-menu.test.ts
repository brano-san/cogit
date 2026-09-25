import { describe, expect, it } from "vitest";
import type { ContextItem, WorktreeEntry } from "./ipc";
import { parseWorktreeCommand, pruneBlocked, worktreeMenu } from "./worktree-menu";

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
  hasSubmodules: false,
};

const find = (items: ContextItem[], id: string) => items.find((item) => item.id === id);
const labels = (items: ContextItem[]) => items.map((item) => (item.separator ? "—" : item.label));

// `popup_context_menu` returns nothing: the choice arrives later as a menu command. Its
// bare `open` was the palette's Open Repository…, and every other item did nothing.
describe("the menu of a Worktrees row", () => {
  it("names every item so that it comes back as a worktree command", () => {
    for (const entry of [LINKED, { ...LINKED, missing: true }, { ...LINKED, locked: "" }]) {
      for (const item of worktreeMenu(entry)) {
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

  it("offers Open, Reveal, Copy Path, Lock and Remove for a linked worktree", () => {
    expect(labels(worktreeMenu(LINKED))).toEqual([
      "Open",
      "Reveal in File Manager",
      "Copy Path",
      "—",
      "Lock",
      "Remove…",
    ]);
    expect(parseWorktreeCommand(find(worktreeMenu(LINKED), "worktree-row-open")!.id)).toBe("open");
  });

  it("offers Unlock instead of Lock once it is locked", () => {
    const items = worktreeMenu({ ...LINKED, locked: "on a USB disk" });
    expect(find(items, "worktree-row-unlock")?.enabled).toBe(true);
    expect(find(items, "worktree-row-lock")).toBeUndefined();
  });
});

// `git worktree remove --force` refuses a locked worktree, and a missing one had no
// Unlock anywhere: it could not be got rid of from Cogit at all.
describe("a missing worktree that is locked", () => {
  const gone = { ...LINKED, missing: true, locked: "" };

  it("can be unlocked from its menu", () => {
    expect(find(worktreeMenu(gone), "worktree-row-unlock")?.enabled).toBe(true);
  });

  it("cannot be pruned until it is, and says why", () => {
    const prune = find(worktreeMenu(gone), "worktree-row-prune");
    expect(prune?.enabled).toBe(false);
    expect(prune?.label).toContain("unlock");
    expect(pruneBlocked(gone)).not.toBeNull();
    expect(pruneBlocked({ ...gone, locked: null })).toBeNull();
  });

  it("offers no Unlock while it is not locked", () => {
    expect(find(worktreeMenu({ ...LINKED, missing: true }), "worktree-row-unlock")).toBeUndefined();
    expect(find(worktreeMenu({ ...LINKED, missing: true }), "worktree-row-prune")?.enabled).toBe(true);
  });
});
