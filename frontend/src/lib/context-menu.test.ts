import { describe, expect, it } from "vitest";
import { SEPARATOR, fileMenu, item, offer, refMenu, repoMenu, tidy } from "./context-menu";

describe("tidy", () => {
  const sep = SEPARATOR;
  const a = item("a", "A");
  const b = item("b", "B");

  it("drops leading, trailing and doubled separators", () => {
    expect(tidy([sep, a, sep, sep, b, sep])).toEqual([a, sep, b]);
  });

  it("leaves nothing of a menu that is only separators", () => {
    expect(tidy([sep, sep])).toEqual([]);
  });
});

describe("offer", () => {
  it("is an ordinary row when nothing blocks it", () => {
    expect(offer("x", "Squash", null)).toEqual(item("x", "Squash"));
  });

  it("puts the reason into the label of a row that is off", () => {
    const off = offer("x", "Squash", "already pushed", "CmdOrCtrl+Q");
    expect(off.enabled).toBe(false);
    expect(off.label).toBe("Squash (already pushed)");
    expect(off.accelerator).toBe("CmdOrCtrl+Q");
  });
});

describe("refMenu", () => {
  const ids = (items: ReturnType<typeof refMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);

  it("offers a lost commit the way back", () => {
    expect(ids(refMenu({ kind: "lost" }))).toEqual(["restore-lost", "copy-sha"]);
  });

  it("has nothing to offer for a heading", () => {
    expect(refMenu({ kind: "group" })).toEqual([]);
  });
});

describe("repoMenu", () => {
  const ids = (items: ReturnType<typeof repoMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);

  it("offers the system actions a repository row needs", () => {
    expect(ids(repoMenu({ active: false }))).toEqual([
      "repo-open",
      "repo-explorer",
      "repo-terminal",
      "repo-copy-path",
      "repo-close",
    ]);
  });

  it("does not offer to open the repository that is already open", () => {
    const menu = repoMenu({ active: true });
    expect(menu.find((entry) => entry.id === "repo-open")?.enabled).toBe(false);
  });

  it("offers to open one that is not", () => {
    const menu = repoMenu({ active: false });
    expect(menu.find((entry) => entry.id === "repo-open")?.enabled).toBe(true);
  });

  it("always allows closing, including the active one", () => {
    for (const active of [true, false]) {
      expect(repoMenu({ active }).find((entry) => entry.id === "repo-close")?.enabled).toBe(true);
    }
  });
});

describe("fileMenu", () => {
  const ids = (items: ReturnType<typeof fileMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);
  const enabled = (items: ReturnType<typeof fileMenu>, id: string) =>
    items.find((entry) => entry.id === id)?.enabled;

  it("offers Stage on an unstaged change and Unstage on a staged one", () => {
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-stage")).toBe(true);
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-unstage")).toBe(
      false,
    );
    expect(enabled(fileMenu({ status: "modified", staged: true, count: 1, worktree: true }), "file-unstage")).toBe(true);
  });

  it("carries the chord the keyboard uses for the same thing", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: true });
    expect(menu.find((entry) => entry.id === "file-stage")?.accelerator).toBe("CmdOrCtrl+T");
    expect(menu.find((entry) => entry.id === "file-discard")?.accelerator).toBe("CmdOrCtrl+Z");
  });

  it("does not offer to discard an untracked file, which Git cannot restore", () => {
    expect(enabled(fileMenu({ status: "untracked", staged: false, count: 1, worktree: true }), "file-discard")).toBe(
      false,
    );
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-discard")).toBe(
      true,
    );
  });

  it("offers Ignore and Delete only for what Git is not tracking", () => {
    const untracked = fileMenu({ status: "untracked", staged: false, count: 1, worktree: true });
    expect(enabled(untracked, "file-ignore")).toBe(true);
    expect(enabled(untracked, "file-delete")).toBe(true);
    const tracked = fileMenu({ status: "modified", staged: false, count: 1, worktree: true });
    expect(enabled(tracked, "file-ignore")).toBe(false);
    expect(enabled(tracked, "file-delete")).toBe(false);
  });

  it("cannot blame a file that has no history yet or is gone", () => {
    expect(enabled(fileMenu({ status: "untracked", staged: false, count: 1, worktree: true }), "file-blame")).toBe(
      false,
    );
    expect(enabled(fileMenu({ status: "deleted", staged: false, count: 1, worktree: true }), "file-blame")).toBe(false);
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-blame")).toBe(true);
  });

  it("turns off what only makes sense for one file once several are marked", () => {
    const many = fileMenu({ status: "modified", staged: false, count: 3, worktree: true });
    expect(enabled(many, "file-blame")).toBe(false);
    expect(enabled(many, "file-explorer")).toBe(false);
    expect(enabled(many, "file-stage")).toBe(true);
  });

  it("puts staging first and the destructive rows behind a separator", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: true });
    expect(ids(menu)[0]).toBe("file-stage");
    const discard = menu.findIndex((entry) => entry.id === "file-discard");
    expect(menu.slice(0, discard).some((entry) => entry.separator)).toBe(true);
  });

  it("never draws two separators in a row or ends on one", () => {
    for (const status of ["modified", "untracked", "deleted", "conflicted"] as const) {
      const menu = fileMenu({ status, staged: false, count: 1, worktree: true });
      expect(menu.at(0)?.separator).toBe(false);
      expect(menu.at(-1)?.separator).toBe(false);
      expect(menu.some((entry, at) => entry.separator && menu[at + 1]?.separator)).toBe(false);
    }
  });
});

describe("fileMenu for a commit", () => {
  it("leaves out everything that would touch the working tree", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: false });
    const ids = menu.map((entry) => entry.id);
    expect(ids).not.toContain("file-stage");
    expect(ids).not.toContain("file-discard");
    expect(ids).not.toContain("file-delete");
  });

  it("still offers what reading a file is for", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: false });
    expect(menu.find((entry) => entry.id === "file-blame")?.enabled).toBe(true);
    expect(menu.find((entry) => entry.id === "file-copy-path")?.enabled).toBe(true);
  });
});
