import { describe, expect, it, vi } from "vitest";
import {
  copiedText,
  fileName,
  flagTurnsOn,
  runFileMenuCommand,
  type FileActions,
  type FileScope,
} from "./file-actions";

const WINDOWS = { root: "D:/work/repo", separator: "\\" };
const LINUX = { root: "/home/me/repo", separator: "/" };

function spyActions(): FileActions {
  const names: (keyof FileActions)[] = [
    "openFile",
    "openVersion",
    "reveal",
    "showChanges",
    "compareWithWorkTree",
    "log",
    "blame",
    "investigate",
    "commit",
    "stash",
    "stage",
    "unstage",
    "indexEditor",
    "move",
    "resolve",
    "ignore",
    "discard",
    "remove",
    "trash",
    "saveAs",
    "applyChange",
    "copy",
    "setFlag",
  ];
  return Object.fromEntries(names.map((name) => [name, vi.fn()])) as unknown as FileActions;
}

const SCOPE: FileScope = {
  path: "src/a.ts",
  paths: ["src/a.ts", "b.txt"],
  statuses: ["modified", "deleted"],
  rev: null,
  oldPath: null,
};

describe("copiedText", () => {
  it("copies names, relative paths and absolute paths, one per line", () => {
    const paths = ["src/a.ts", "generated/"];
    expect(copiedText("name", paths, WINDOWS)).toBe("a.ts\ngenerated");
    expect(copiedText("relative", paths, WINDOWS)).toBe("src/a.ts\ngenerated");
    expect(copiedText("path", paths, WINDOWS)).toBe("D:\\work\\repo\\src\\a.ts\nD:\\work\\repo\\generated");
    expect(copiedText("path", ["src/a.ts"], LINUX)).toBe("/home/me/repo/src/a.ts");
  });

  it("reads a file name the same at the root", () => {
    expect(fileName("README.md")).toBe("README.md");
  });
});

describe("flagTurnsOn", () => {
  it("sets the flag unless every file already has it", () => {
    expect(flagTurnsOn(["modified"], "assumeUnchanged")).toBe(true);
    expect(flagTurnsOn(["assumeUnchanged", "assumeUnchanged"], "assumeUnchanged")).toBe(false);
    expect(flagTurnsOn(["assumeUnchanged", "modified"], "assumeUnchanged")).toBe(true);
    expect(flagTurnsOn(["skipped"], "skipWorktree")).toBe(false);
  });
});

describe("runFileMenuCommand", () => {
  it("hands the whole scope to the actions that take many files", () => {
    const actions = spyActions();
    expect(runFileMenuCommand("file-stage", SCOPE, actions, WINDOWS)).toBe(true);
    expect(actions.stage).toHaveBeenCalledWith(["src/a.ts", "b.txt"]);
  });

  it("does not ask the bin for a file that is already gone", () => {
    const actions = spyActions();
    runFileMenuCommand("file-delete", SCOPE, actions, WINDOWS);
    expect(actions.trash).toHaveBeenCalledWith(["src/a.ts"]);
  });

  it("resolves every file with the chosen side", () => {
    const actions = spyActions();
    runFileMenuCommand("file-resolve-theirs", SCOPE, actions, WINDOWS);
    expect(actions.resolve).toHaveBeenCalledWith(["src/a.ts", "b.txt"], "theirs");
  });

  it("cherry-picks forward and reverts backward, with the rename source", () => {
    const actions = spyActions();
    const scope = { ...SCOPE, rev: "abc123", oldPath: "old/a.ts" };
    runFileMenuCommand("file-cherry-pick", scope, actions, WINDOWS);
    runFileMenuCommand("file-revert", scope, actions, WINDOWS);
    expect(actions.applyChange).toHaveBeenNthCalledWith(1, "src/a.ts", "old/a.ts", "abc123", false);
    expect(actions.applyChange).toHaveBeenNthCalledWith(2, "src/a.ts", "old/a.ts", "abc123", true);
  });

  it("does nothing commit-bound without a commit", () => {
    const actions = spyActions();
    runFileMenuCommand("file-open-version", SCOPE, actions, WINDOWS);
    expect(actions.openVersion).not.toHaveBeenCalled();
  });

  it("toggles a flag in the direction the statuses call for", () => {
    const actions = spyActions();
    runFileMenuCommand("file-assume-unchanged", SCOPE, actions, WINDOWS);
    expect(actions.setFlag).toHaveBeenCalledWith(SCOPE.paths, "assumeUnchanged", true);
  });

  it("copies the platform's own absolute path", () => {
    const actions = spyActions();
    runFileMenuCommand("file-copy-path", { ...SCOPE, paths: ["src/a.ts"] }, actions, WINDOWS);
    expect(actions.copy).toHaveBeenCalledWith("D:\\work\\repo\\src\\a.ts");
  });

  it("leaves ids that are not its own alone", () => {
    expect(runFileMenuCommand("repo-pin", SCOPE, spyActions(), WINDOWS)).toBe(false);
  });
});
