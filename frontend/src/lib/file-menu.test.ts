import { describe, expect, it } from "vitest";
import type { ContextItem } from "./ipc";
import { commitFileMenu, worktreeFileMenu, type WorktreeFileTarget } from "./file-menu";

const MODIFIED: WorktreeFileTarget = {
  section: "worktree",
  statuses: ["modified"],
  staged: false,
  unstaged: true,
  fileManager: "Explorer",
};

const shape = (items: ContextItem[]) => items.map((item) => (item.separator ? "—" : item.label));
const find = (items: ContextItem[], id: string) => items.find((item) => item.id === id);
const on = (items: ContextItem[], id: string) => find(items, id)?.enabled;

describe("worktreeFileMenu", () => {
  it("is the list from the task, in its order, with its separators", () => {
    expect(shape(worktreeFileMenu(MODIFIED))).toEqual([
      "Open File",
      "Reveal in Explorer",
      "Show Changes",
      "Log",
      "Blame",
      "Investigate",
      "—",
      "Commit…",
      "Stash Selection…",
      "—",
      "Stage",
      "Unstage",
      "Index Editor…",
      "Move or Rename…",
      "—",
      "Resolve",
      "—",
      "Ignore",
      "Discard…",
      "Remove…",
      "Delete…",
      "—",
      "Copy Name",
      "Copy Path",
      "Copy Relative Path",
      "—",
      "Toggle 'Assume Unchanged'",
      "Toggle 'Skip Worktree'",
    ]);
  });

  it("shows the chords the keyboard uses for the same things", () => {
    const menu = worktreeFileMenu(MODIFIED);
    expect(find(menu, "file-stage")?.accelerator).toBe("CmdOrCtrl+T");
    expect(find(menu, "file-unstage")?.accelerator).toBe("CmdOrCtrl+Shift+T");
    expect(find(menu, "file-discard")?.accelerator).toBe("CmdOrCtrl+Z");
    expect(find(menu, "file-blame")?.accelerator).toBe("CmdOrCtrl+Shift+L");
    // No key reveals a file: the menu must not show one it does not have (R-453).
    expect(find(menu, "file-reveal")?.accelerator ?? null).toBeNull();
  });

  it("stages what is unstaged and unstages what is staged", () => {
    expect(on(worktreeFileMenu(MODIFIED), "file-stage")).toBe(true);
    expect(on(worktreeFileMenu(MODIFIED), "file-unstage")).toBe(false);
    const staged = worktreeFileMenu({ ...MODIFIED, section: "index", staged: true, unstaged: false });
    expect(on(staged, "file-unstage")).toBe(true);
    expect(on(staged, "file-stage")).toBe(false);
  });

  it("offers Resolve with both sides only for a conflicted file", () => {
    const conflicted = worktreeFileMenu({ ...MODIFIED, statuses: ["conflicted"] });
    const resolve = find(conflicted, "file-resolve");
    expect(resolve?.enabled).toBe(true);
    expect(resolve?.children?.map((child) => child.label)).toEqual(["Take Theirs", "Take Ours"]);
    expect(on(worktreeFileMenu(MODIFIED), "file-resolve")).toBe(false);
  });

  it("ignores only untracked files and removes only tracked ones", () => {
    const untracked = worktreeFileMenu({ ...MODIFIED, statuses: ["untracked"] });
    expect(on(untracked, "file-ignore")).toBe(true);
    expect(on(untracked, "file-remove")).toBe(false);
    expect(on(untracked, "file-discard")).toBe(false);
    expect(on(worktreeFileMenu(MODIFIED), "file-ignore")).toBe(false);
    expect(on(worktreeFileMenu(MODIFIED), "file-remove")).toBe(true);
  });

  it("cannot open, reveal or delete a file that is no longer on disk", () => {
    const gone = worktreeFileMenu({ ...MODIFIED, statuses: ["deleted"] });
    expect(on(gone, "file-open")).toBe(false);
    expect(find(gone, "file-reveal")?.label).toBe("Reveal in Explorer (not on disk)");
    expect(on(gone, "file-delete")).toBe(false);
    expect(on(gone, "file-blame")).toBe(true);
  });

  it("has no history to show for a file Git has not recorded yet", () => {
    for (const status of ["untracked", "added"]) {
      const menu = worktreeFileMenu({ ...MODIFIED, statuses: [status] });
      expect(on(menu, "file-log"), status).toBe(false);
      expect(on(menu, "file-blame"), status).toBe(false);
      expect(on(menu, "file-investigate"), status).toBe(false);
    }
  });

  it("turns off what works on one file once several are ticked, and says so", () => {
    const many = worktreeFileMenu({ ...MODIFIED, statuses: ["modified", "modified", "added"] });
    expect(find(many, "file-open")?.label).toBe("Open File (one file only)");
    expect(on(many, "file-index-editor")).toBe(false);
    expect(on(many, "file-move")).toBe(false);
    expect(on(many, "file-stage")).toBe(true);
    expect(on(many, "file-copy-path")).toBe(true);
  });

  it("offers the two index flags on the working tree list and explains them away on the index", () => {
    expect(on(worktreeFileMenu(MODIFIED), "file-assume-unchanged")).toBe(true);
    const index = worktreeFileMenu({ ...MODIFIED, section: "index", staged: true });
    expect(find(index, "file-skip-worktree")?.label).toBe("Toggle 'Skip Worktree' (working tree only)");
    expect(on(worktreeFileMenu({ ...MODIFIED, statuses: ["untracked"] }), "file-assume-unchanged")).toBe(false);
  });

  it("names the file manager of the platform", () => {
    expect(find(worktreeFileMenu({ ...MODIFIED, fileManager: "File Manager" }), "file-reveal")?.label).toBe(
      "Reveal in File Manager",
    );
  });
});

describe("commitFileMenu", () => {
  const base = { status: "modified", count: 1, onDisk: true, fileManager: "Explorer" };

  it("is the list from the task, in its order, with its separators", () => {
    expect(shape(commitFileMenu(base))).toEqual([
      "Show Changes",
      "Compare with Working Tree",
      "Open File",
      "Reveal in Explorer",
      "—",
      "Save As…",
      "Log",
      "Blame",
      "Investigate",
      "—",
      "Cherry-Pick",
      "Revert",
      "—",
      "Copy Path",
      "Copy Relative Path",
      "Copy Name",
    ]);
  });

  it("does not reveal a file that is gone from the working tree", () => {
    const item = find(commitFileMenu({ ...base, onDisk: false }), "file-reveal");
    expect(item).toMatchObject({ enabled: false, label: "Reveal in Explorer (not in the working tree)" });
  });

  it("has no version to open, save or blame when the commit deleted the file", () => {
    const menu = commitFileMenu({ ...base, status: "deleted" });
    for (const id of ["file-open-version", "file-save-as", "file-blame", "file-investigate"]) {
      expect(on(menu, id), id).toBe(false);
    }
    expect(on(menu, "file-changes")).toBe(true);
    expect(on(menu, "file-revert")).toBe(true);
  });

  it("works on one file at a time, except for copying", () => {
    const many = commitFileMenu({ ...base, count: 2 });
    expect(on(many, "file-cherry-pick")).toBe(false);
    expect(on(many, "file-compare-worktree")).toBe(false);
    expect(on(many, "file-copy-name")).toBe(true);
  });
});
