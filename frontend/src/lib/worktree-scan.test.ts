import { describe, expect, it } from "vitest";
import type { FileEntry, UnpushedInSubmodule, WorktreeEntry } from "$lib/ipc";
import {
  applyChunk,
  removeButton,
  scanSettled,
  stageRows,
  startScan,
  uncommittedList,
  unpushedTotal,
} from "./worktree-scan";

const file = (path: string, over: Partial<FileEntry> = {}): FileEntry => ({
  path,
  oldPath: null,
  status: "modified",
  mode: "plain",
  modeChange: null,
  similarity: null,
  submodule: null,
  conflict: null,
  ...over,
});

const entry = (over: Partial<WorktreeEntry> = {}): WorktreeEntry => ({
  path: "E:/wt",
  name: "wt",
  branch: "feat",
  head: "a".repeat(40),
  isMain: false,
  isCurrent: false,
  locked: null,
  missing: false,
  dirty: false,
  hasSubmodules: false,
  ...over,
});

const unpushed = (path: string, total: number): UnpushedInSubmodule => ({
  path,
  total,
  commits: [{ oid: "b".repeat(40), summary: "local" }],
});

describe("worktree removal scan", () => {
  it("starts with every stage running", () => {
    expect(stageRows(startScan()).map((row) => row.status)).toEqual(["running", "running", "running"]);
    expect(scanSettled(startScan())).toBe(false);
  });

  it("names the stages in the order they are shown", () => {
    expect(stageRows(startScan()).map((row) => row.label)).toEqual([
      "Changes",
      "Submodules",
      "Unpushed commits",
    ]);
  });

  it("settles each stage on its own, in any order", () => {
    let scan = startScan();
    scan = applyChunk(scan, { kind: "unpushed", found: [] });
    expect(stageRows(scan).map((row) => row.status)).toEqual(["running", "running", "done"]);
    scan = applyChunk(scan, { kind: "changes", files: [file("a.txt")] });
    scan = applyChunk(scan, { kind: "submodules", modules: { paths: [], changed: [] } });
    expect(scanSettled(scan)).toBe(true);
  });

  it("ignores the started and done chunks", () => {
    const scan = startScan();
    expect(applyChunk(scan, { kind: "started", id: 3 })).toBe(scan);
    expect(applyChunk(scan, { kind: "done", cancelled: false })).toBe(scan);
  });

  it("keeps a failed stage failed, with its reason", () => {
    const scan = applyChunk(startScan(), {
      kind: "failed",
      stage: "unpushed",
      error: { kind: "internal", data: "boom" },
    });
    const row = stageRows(scan)[2]!;
    expect(row.status).toBe("failed");
    expect(row.detail).toContain("boom");
  });

  it("describes what each finished stage found", () => {
    let scan = applyChunk(startScan(), { kind: "changes", files: [file("a"), file("b")] });
    scan = applyChunk(scan, {
      kind: "submodules",
      modules: { paths: ["x", "y"], changed: [] },
    });
    scan = applyChunk(scan, { kind: "unpushed", found: [unpushed("x", 3)] });
    expect(stageRows(scan).map((row) => row.detail)).toEqual([
      "2 files",
      "2 checked out",
      "3 in 1 submodule",
    ]);
  });

  it("reads clean stages as none", () => {
    let scan = applyChunk(startScan(), { kind: "changes", files: [] });
    scan = applyChunk(scan, { kind: "submodules", modules: { paths: [], changed: [] } });
    scan = applyChunk(scan, { kind: "unpushed", found: [] });
    expect(stageRows(scan).map((row) => row.detail)).toEqual(["none", "none", "none"]);
  });

  it("lists changes and changed submodules together, sorted by path", () => {
    let scan = applyChunk(startScan(), { kind: "changes", files: [file("z.txt"), file("a.txt")] });
    scan = applyChunk(scan, {
      kind: "submodules",
      modules: { paths: ["vendor/lib"], changed: [file("vendor/lib", { mode: "submodule" })] },
    });
    expect(uncommittedList(scan)?.map((row) => row.path)).toEqual(["a.txt", "vendor/lib", "z.txt"]);
  });

  it("has no list until both stages that feed it are in", () => {
    const scan = applyChunk(startScan(), { kind: "changes", files: [file("a")] });
    expect(uncommittedList(scan)).toBeNull();
  });

  it("totals unpushed commits over submodules", () => {
    const scan = applyChunk(startScan(), {
      kind: "unpushed",
      found: [unpushed("a", 2), unpushed("b", 5)],
    });
    expect(unpushedTotal(scan)).toBe(7);
  });
});

describe("remove button", () => {
  const settled = () => {
    let scan = applyChunk(startScan(), { kind: "changes", files: [] });
    scan = applyChunk(scan, { kind: "submodules", modules: { paths: [], changed: [] } });
    return applyChunk(scan, { kind: "unpushed", found: [] });
  };

  it("waits for the check", () => {
    expect(removeButton(entry(), startScan(), false)).toEqual({
      label: "Remove",
      disabled: true,
      tip: "Waiting for the check",
    });
  });

  it("says when the check failed", () => {
    const scan = applyChunk(settled(), {
      kind: "failed",
      stage: "changes",
      error: { kind: "internal", data: "x" },
    });
    const button = removeButton(entry(), scan, false);
    expect(button.disabled).toBe(true);
    expect(button.tip).toBe("The check failed");
  });

  it("is open for a clean worktree", () => {
    expect(removeButton(entry(), settled(), false)).toEqual({
      label: "Remove",
      disabled: false,
      tip: undefined,
    });
  });

  it("needs the --force box for changes", () => {
    const scan = applyChunk(settled(), { kind: "changes", files: [file("a")] });
    expect(removeButton(entry(), scan, false)).toEqual({
      label: "Remove with --force",
      disabled: true,
      tip: "Tick Remove anyway (--force) first",
    });
    expect(removeButton(entry(), scan, true).disabled).toBe(false);
  });

  it("needs the box for submodules even when everything is clean", () => {
    const button = removeButton(entry({ hasSubmodules: true }), settled(), false);
    expect(button.label).toBe("Remove with --force");
    expect(button.disabled).toBe(true);
  });
});
