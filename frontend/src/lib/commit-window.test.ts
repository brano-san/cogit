import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import {
  amendMessage,
  counter,
  listedFiles,
  menuLabel,
  parseCommitWindow,
  planCommit,
  ready,
  recentMessages,
  sortedFiles,
  splitPath,
  stateSide,
} from "./commit-window";

const file = (path: string, status: FileEntry["status"] = "modified"): FileEntry => ({
  path,
  oldPath: null,
  status,
  mode: "plain",
  modeChange: null,
  similarity: null,
});

describe("parseCommitWindow", () => {
  it("reads the repository and the root", () => {
    expect(parseCommitWindow("?repo=3&root=D%3A%5Cmy%20repo")).toEqual({ repo: 3, root: "D:\\my repo" });
  });
  it("refuses a window without either", () => {
    expect(parseCommitWindow("?repo=0&root=x")).toBeNull();
    expect(parseCommitWindow("?repo=3")).toBeNull();
  });
});

describe("listedFiles", () => {
  const staged = [file("a.txt"), file("b.txt")];
  const unstaged = [file("b.txt"), file("c.txt", "untracked")];
  it("lists only staged files in staged mode", () => {
    expect(listedFiles("staged", staged, unstaged).map((f) => f.path)).toEqual(["a.txt", "b.txt"]);
  });
  it("lists a partly staged file once in local mode", () => {
    expect(listedFiles("local", staged, unstaged).map((f) => f.path)).toEqual(["a.txt", "b.txt", "c.txt"]);
  });
});

describe("stateSide", () => {
  const staged = [file("a.txt"), file("b.txt")];
  const unstaged = [file("b.txt"), file("c.txt", "untracked")];
  it("reads the index side of everything in staged mode", () => {
    expect(stateSide("staged", "a.txt", staged, unstaged)).toBe("index");
  });
  it("reads a file only in the index as staged in local mode", () => {
    expect(stateSide("local", "a.txt", staged, unstaged)).toBe("index");
  });
  it("reads a partly staged file by what is left to stage", () => {
    expect(stateSide("local", "b.txt", staged, unstaged)).toBe("worktree");
  });
  it("reads an unstaged file by the working tree", () => {
    expect(stateSide("local", "c.txt", staged, unstaged)).toBe("worktree");
  });
});

describe("sortedFiles", () => {
  const files = [file("z/b.txt"), file("a/c.txt"), file("m.txt")];
  it("sorts by name, then reverses", () => {
    expect(sortedFiles(files, "name", false).map((f) => splitPath(f.path).name)).toEqual(["b.txt", "c.txt", "m.txt"]);
    expect(sortedFiles(files, "name", true)[0]?.path).toBe("m.txt");
  });
  it("sorts by directory, the root first", () => {
    expect(sortedFiles(files, "directory", false).map((f) => f.path)).toEqual(["m.txt", "a/c.txt", "z/b.txt"]);
  });
});

describe("counter", () => {
  it("counts a single modified file the SmartGit way", () => {
    expect(counter([file("a")])).toBe("1 file (~1)");
  });
  it("breaks several files down by kind", () => {
    expect(counter([file("a", "added"), file("b", "untracked"), file("c"), file("d", "deleted")])).toBe(
      "4 files (+2 ~1 -1)",
    );
  });
  it("says so when nothing is selected", () => {
    expect(counter([])).toBe("0 files");
  });
});

describe("planCommit", () => {
  const staged = [file("a"), file("b")];
  it("commits everything staged when every staged file is ticked", () => {
    expect(planCommit("staged", ["a", "b"], staged)).toEqual({ stage: [], only: [] });
  });
  it("narrows a staged commit to the ticked files", () => {
    expect(planCommit("staged", ["a"], staged)).toEqual({ stage: [], only: ["a"] });
  });
  it("stages the local files that are not in the index and commits exactly the ticked ones", () => {
    expect(planCommit("local", ["a", "c"], staged)).toEqual({ stage: ["c"], only: ["a", "c"] });
  });
});

describe("ready", () => {
  const base = {
    message: "fix",
    template: null,
    ticked: 1,
    stagedTotal: 1,
    amend: false,
    unborn: false,
    busy: false,
    committing: false,
  };
  it("needs a message and a ticked file", () => {
    expect(ready(base)).toBe(true);
    expect(ready({ ...base, message: " " })).toBe(false);
    expect(ready({ ...base, ticked: 0 })).toBe(false);
  });
  it("lets Amend reword when nothing is staged at all", () => {
    expect(ready({ ...base, ticked: 0, stagedTotal: 0, amend: true })).toBe(true);
  });
  it("does not let Amend with no ticked file commit the staged ones", () => {
    expect(ready({ ...base, ticked: 0, stagedTotal: 2, amend: true })).toBe(false);
  });
});

describe("messages", () => {
  const log = [
    { summary: "b", body: "more" },
    { summary: "a", body: "" },
    { summary: "a", body: "" },
  ];
  it("offers each message once, whole, newest first", () => {
    expect(recentMessages(log)).toEqual(["b\n\nmore", "a"]);
  });
  it("cuts a long subject for the row", () => {
    expect(menuLabel("x".repeat(70) + "\nbody").endsWith("…")).toBe(true);
    expect(menuLabel("short\nbody")).toBe("short");
  });
});

describe("amendMessage", () => {
  it("takes the last message and remembers what was typed", () => {
    expect(amendMessage(true, "draft", "last", null)).toEqual({ message: "last", before: "draft" });
  });
  it("gives the draft back when the message was not touched", () => {
    expect(amendMessage(false, "last", "last", "draft")).toEqual({ message: "draft", before: null });
  });
  it("keeps an edited message", () => {
    expect(amendMessage(false, "last, edited", "last", "draft")).toEqual({
      message: "last, edited",
      before: null,
    });
  });
});
