import { describe, expect, it } from "vitest";
import type { ContextItem, Submodule } from "./ipc";
import type { DesktopInfo } from "./ipc/file-menus";
import {
  NOT_A_SUBMODULE,
  REPO_MOVE_NEW,
  SUBMODULE_REASON,
  groupChoices,
  parseRepoCommand,
  repoMenu,
  type RepoMenuTarget,
} from "./repo-menu";
import { UNGROUPED } from "./repo-groups";

const WINDOWS: DesktopInfo = {
  fileManager: "Explorer",
  windowsShells: true,
  gitShell: "C:/Program Files/Git/git-bash.exe",
  separator: "\\",
};
const LINUX: DesktopInfo = { fileManager: "File Manager", windowsShells: false, gitShell: null, separator: "/" };

const OPEN: RepoMenuTarget = {
  kind: "repository",
  active: false,
  open: true,
  missing: false,
  pinned: false,
  group: UNGROUPED,
  groups: [
    { id: "g1", name: "Work", depth: 0 },
    { id: "g2", name: "Clients", depth: 1 },
  ],
};

const module = (over: Partial<Submodule>): Submodule => ({
  name: "lib",
  path: "lib",
  url: "https://example.invalid/lib.git",
  recorded: "452e8002c0ffee0000000000000000000000beef",
  checkedOut: "452e8002c0ffee0000000000000000000000beef",
  state: "inSync",
  branch: null,
  subject: null,
  nested: false,
  ahead: 0,
  behind: 0,
  repoState: null,
  ...over,
});

const shape = (items: ContextItem[]) => items.map((item) => (item.separator ? "—" : item.label));
const find = (items: ContextItem[], id: string) => items.find((item) => item.id === id);

describe("repoMenu", () => {
  it("is SmartGit's list, in its order, with its separators", () => {
    expect(shape(repoMenu(OPEN, WINDOWS))).toEqual([
      "Open Repository",
      "Open in Explorer",
      "Reveal in Explorer",
      "Open in Terminal",
      "Open in PowerShell",
      "Open in Git Shell",
      "Close Repository",
      "—",
      "Pull",
      "Push",
      `Update (${NOT_A_SUBMODULE})`,
      "—",
      "Move To",
      "Pin",
      "Rename…",
      "Remove…",
    ]);
  });

  it("offers Update on a submodule that is not where its parent records it", () => {
    const at = (over: Partial<Submodule>) => ({ ...OPEN, kind: "submodule" as const, groups: [], module: module(over) });
    expect(find(repoMenu(at({ state: "behind", behind: 1 }), WINDOWS), "repo-update")).toMatchObject({
      enabled: true,
      label: "Update",
    });
    expect(find(repoMenu(at({ state: "ahead", ahead: 1 }), WINDOWS), "repo-update")?.enabled).toBe(true);
    const synced = find(repoMenu(at({}), WINDOWS), "repo-update");
    expect(synced?.enabled).toBe(false);
    expect(synced?.label).toMatch(/^Update \(.+\)$/);
  });

  it("calls the file manager by its Linux name and leaves the Windows shells out there", () => {
    const labels = shape(repoMenu(OPEN, LINUX));
    expect(labels).toContain("Open in File Manager");
    expect(labels).toContain("Reveal in File Manager");
    expect(labels).not.toContain("Open in PowerShell");
    expect(labels).not.toContain("Open in Git Shell");
  });

  it("keeps Git Shell on Windows but disabled when Git Bash is not installed", () => {
    const item = find(repoMenu(OPEN, { ...WINDOWS, gitShell: null }), "repo-git-shell");
    expect(item).toMatchObject({ enabled: false, label: "Open in Git Shell (Git Bash not found)" });
  });

  it("does not open what is already open in the window", () => {
    expect(find(repoMenu({ ...OPEN, active: true }, WINDOWS), "repo-open")?.enabled).toBe(false);
    expect(find(repoMenu(OPEN, WINDOWS), "repo-open")?.enabled).toBe(true);
  });

  it("does not close, pull or push a closed repository", () => {
    const menu = repoMenu({ ...OPEN, open: false }, WINDOWS);
    expect(find(menu, "repo-close")?.enabled).toBe(false);
    expect(find(menu, "repo-pull")?.enabled).toBe(false);
    expect(find(menu, "repo-push")?.enabled).toBe(false);
    expect(find(menu, "repo-open")?.enabled).toBe(true);
  });

  it("shows the chords only on the repository they act on", () => {
    expect(find(repoMenu({ ...OPEN, active: true }, WINDOWS), "repo-pull")?.accelerator).toBe("CmdOrCtrl+Shift+U");
    expect(find(repoMenu(OPEN, WINDOWS), "repo-pull")?.accelerator).toBeNull();
  });

  it("says Unpin on a pinned row", () => {
    expect(find(repoMenu({ ...OPEN, pinned: true }, WINDOWS), "repo-pin")?.label).toBe("Unpin");
  });

  it("offers every group to move to, except the one it is in, and a new one", () => {
    const move = find(repoMenu({ ...OPEN, group: "g1" }, WINDOWS), "repo-move");
    expect(move?.children?.map((child) => (child.separator ? "—" : [child.label, child.enabled]))).toEqual([
      ["No Group", true],
      ["Work", false],
      ["    Clients", true],
      "—",
      ["New Group…", true],
    ]);
  });

  it("disables the list-only items of a submodule and says why", () => {
    const menu = repoMenu({ ...OPEN, kind: "submodule", groups: [] }, WINDOWS);
    for (const id of ["repo-move", "repo-pin", "repo-rename", "repo-remove"]) {
      const item = find(menu, id);
      expect(item?.enabled, id).toBe(false);
      expect(item?.label, id).toContain(SUBMODULE_REASON);
      expect(item?.children, id).toBeUndefined();
    }
    expect(find(menu, "repo-reveal")?.enabled).toBe(true);
  });

  it("disables what needs the folder when the folder is missing", () => {
    const menu = repoMenu({ ...OPEN, missing: true }, WINDOWS);
    expect(find(menu, "repo-open-folder")?.enabled).toBe(false);
    expect(find(menu, "repo-reveal")?.label).toContain("missing");
    expect(find(menu, "repo-remove")?.enabled).toBe(true);
  });
});

describe("groupChoices", () => {
  it("walks the groups in tree order with their depth", () => {
    const choices = groupChoices({
      order: ["a", "b", "c"],
      names: { a: "Work", b: "Home", c: "Clients" },
      of: {},
      under: { c: "a" },
    });
    expect(choices).toEqual([
      { id: "a", name: "Work", depth: 0 },
      { id: "c", name: "Clients", depth: 1 },
      { id: "b", name: "Home", depth: 0 },
    ]);
  });
});

describe("parseRepoCommand", () => {
  it("reads the group out of a Move To item", () => {
    expect(parseRepoCommand("repo-move:g2")).toEqual({ kind: "move", group: "g2" });
    expect(parseRepoCommand("repo-move:")).toEqual({ kind: "move", group: UNGROUPED });
    expect(parseRepoCommand(REPO_MOVE_NEW)).toEqual({ kind: "move-new" });
  });

  it("passes the other repository items through and ignores the rest", () => {
    expect(parseRepoCommand("repo-pin")).toEqual({ kind: "plain", id: "repo-pin" });
    expect(parseRepoCommand("file-stage")).toBeNull();
  });
});
