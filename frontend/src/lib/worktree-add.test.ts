import { describe, expect, it } from "vitest";
import type { Branch } from "$lib/ipc";
import {
  addProblem,
  addRequest,
  commandPreview,
  defaultBase,
  folderLabel,
  localNameOfRemote,
  remoteBase,
  suggestFolder,
  type AddForm,
} from "./worktree-add";

const b = (name: string, kind: "local" | "remote" = "local"): Branch => ({ name, kind }) as Branch;
const branches = [b("main"), b("dev"), b("origin/master", "remote"), b("origin/feature/x", "remote")];
const root = "C:/Work/dtv_device";

const form = (patch: Partial<AddForm> = {}): AddForm => ({
  mode: "new",
  name: "feature/x",
  base: "origin/master",
  track: true,
  existing: "",
  folder: "C:/Work/dtv_device-feature-x",
  ...patch,
});

describe("suggestFolder", () => {
  it("puts <repo>-<label> next to the repository, slashes become dashes", () => {
    expect(suggestFolder("C:/Work/dtv_device", "feature/x")).toBe("C:/Work/dtv_device-feature-x");
    expect(suggestFolder("C:\\Work\\dtv_device\\", "feature/x")).toBe("C:/Work/dtv_device-feature-x");
    expect(suggestFolder("/home/me/app", "fix:1 a")).toBe("/home/me/app-fix-1-a");
  });
  it("suggests nothing without a label", () => {
    expect(suggestFolder("C:/Work/app", "")).toBe("");
  });
});

describe("folderLabel", () => {
  it("is the new name, the local name of an existing branch, or the revision", () => {
    expect(folderLabel(form(), branches)).toBe("feature/x");
    expect(folderLabel(form({ mode: "existing", existing: "origin/feature/x" }), branches)).toBe("feature/x");
    expect(folderLabel(form({ mode: "existing", existing: "dev" }), branches)).toBe("dev");
    expect(folderLabel(form({ mode: "detached", base: "origin/master" }), branches)).toBe("origin/master");
    expect(folderLabel(form({ mode: "detached", base: "a".repeat(40) }), branches)).toBe("a".repeat(7));
  });
});

describe("remote branches", () => {
  it("knows a remote base and its local name", () => {
    expect(remoteBase("origin/master", branches)).toBe("origin/master");
    expect(remoteBase("main", branches)).toBeNull();
    expect(remoteBase("HEAD", branches)).toBeNull();
    expect(localNameOfRemote("origin/feature/x")).toBe("feature/x");
  });
});

describe("defaultBase", () => {
  it("is the selected commit, the clicked branch or the current branch", () => {
    expect(defaultBase({ kind: "commit", oid: "abc" }, "main")).toBe("abc");
    expect(defaultBase({ kind: "branch", name: "dev" }, "main")).toBe("dev");
    expect(defaultBase({ kind: "current" }, "main")).toBe("main");
    expect(defaultBase({ kind: "current" }, null)).toBe("HEAD");
  });
});

describe("addRequest", () => {
  it("new branch from a remote branch tracks only when asked", () => {
    expect(addRequest(form(), branches).branch).toEqual({
      kind: "new",
      name: "feature/x",
      start: "origin/master",
      track: true,
    });
    expect(addRequest(form({ track: false }), branches).branch).toMatchObject({ track: false });
    expect(addRequest(form({ base: "main" }), branches).branch).toMatchObject({ track: false });
  });
  it("an empty base means HEAD", () => {
    expect(addRequest(form({ base: " " }), branches).branch).toMatchObject({ start: null });
  });
  it("existing remote branch becomes a tracking branch of its local name", () => {
    expect(addRequest(form({ mode: "existing", existing: "origin/feature/x" }), branches).branch).toEqual({
      kind: "new",
      name: "feature/x",
      start: "origin/feature/x",
      track: true,
    });
    expect(addRequest(form({ mode: "existing", existing: "dev" }), branches).branch).toEqual({
      kind: "existing",
      name: "dev",
    });
  });
  it("detached carries the revision", () => {
    expect(addRequest(form({ mode: "detached", base: "HEAD~2" }), branches).branch).toEqual({
      kind: "detached",
      start: "HEAD~2",
    });
  });
});

describe("commandPreview", () => {
  it("shows the command as it will run, the sibling folder relative", () => {
    expect(commandPreview(form(), branches, root)).toBe(
      "git worktree add --track -b feature/x ../dtv_device-feature-x origin/master",
    );
  });
  it("spells out --no-track only for a remote base", () => {
    expect(commandPreview(form({ track: false }), branches, root)).toContain("--no-track -b");
    expect(commandPreview(form({ base: "main" }), branches, root)).toBe(
      "git worktree add -b feature/x ../dtv_device-feature-x main",
    );
  });
  it("covers existing and detached, and quotes what has spaces", () => {
    expect(commandPreview(form({ mode: "existing", existing: "dev" }), branches, root)).toBe(
      "git worktree add ../dtv_device-feature-x dev",
    );
    expect(commandPreview(form({ mode: "detached", base: "HEAD~2", folder: "D:/my wt" }), branches, root)).toBe(
      'git worktree add --detach "D:/my wt" HEAD~2',
    );
  });
});

describe("addProblem", () => {
  const checks = (patch: Record<string, unknown> = {}) => ({
    base: { rev: "origin/master", problem: null as string | null },
    name: { name: "feature/x", problem: null as string | null },
    folder: { path: "C:/Work/dtv_device-feature-x", problem: null as string | null },
    ...patch,
  });

  it("is clear when every answer is in and good", () => {
    expect(addProblem(form(), branches, checks())).toEqual({ text: null, pending: false });
  });
  it("reports the first problem in form order: name, base, folder", () => {
    const all = checks({
      name: { name: "feature/x", problem: "bad name" },
      base: { rev: "origin/master", problem: "No commit" },
      folder: { path: "C:/Work/dtv_device-feature-x", problem: "not empty" },
    });
    expect(addProblem(form(), branches, all).text).toBe("bad name");
    expect(addProblem(form(), branches, { ...all, name: { name: "feature/x", problem: null } }).text).toBe(
      "No commit",
    );
  });
  it("refuses an empty or taken name without waiting for git", () => {
    expect(addProblem(form({ name: "" }), branches, checks()).text).toContain("name");
    expect(addProblem(form({ name: "dev" }), branches, checks()).text).toBe("A branch named dev already exists");
  });
  it("is pending while an answer belongs to older input", () => {
    expect(addProblem(form({ base: "origin/maste" }), branches, checks()).pending).toBe(true);
    expect(addProblem(form({ name: "feature/xy" }), branches, checks()).pending).toBe(true);
    expect(addProblem(form({ folder: "C:/other" }), branches, checks()).pending).toBe(true);
    expect(addProblem(form(), branches, checks({ folder: null })).pending).toBe(true);
  });
  it("needs a folder", () => {
    expect(addProblem(form({ folder: "" }), branches, checks()).text).toContain("folder");
  });
  it("existing branch must be chosen, and not held elsewhere", () => {
    expect(addProblem(form({ mode: "existing", existing: "" }), branches, checks()).text).toBe("Choose a branch");
    expect(addProblem(form({ mode: "existing", existing: "nope" }), branches, checks()).text).toBe(
      "Choose a branch",
    );
    const held = new Map([["dev", "C:/wt/dev"]]);
    expect(addProblem(form({ mode: "existing", existing: "dev" }), branches, checks(), held).text).toBe(
      "dev is checked out in C:/wt/dev",
    );
    expect(addProblem(form({ mode: "existing", existing: "dev" }), branches, checks()).text).toBeNull();
  });
  it("detached needs only a base and a folder", () => {
    expect(addProblem(form({ mode: "detached", name: "" }), branches, checks()).text).toBeNull();
  });
});
