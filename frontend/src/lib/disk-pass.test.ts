import { describe, expect, it } from "vitest";
import type { ChangeKind, RepoId } from "./ipc";
import { runDiskPass, type DiskPassContext } from "./disk-pass";

const STATE = { status: { staged: 0, unstaged: 1, untracked: 0, conflicted: 0 }, conflicted: [], indexLock: null };

function context(over: Partial<DiskPassContext> = {}) {
  const log: string[] = [];
  const states: unknown[] = [];
  const note = (name: string) => () => {
    log.push(name);
    return Promise.resolve();
  };
  const base: DiskPassContext = {
    repo: () => "repo" as unknown as RepoId,
    epoch: () => 1,
    commitSelected: () => false,
    refreshRefs: note("refreshRefs"),
    resetProtection: () => log.push("resetProtection"),
    hooksOpen: () => false,
    refreshHooks: () => log.push("refreshHooks"),
    loadWorktree: () => {
      log.push("loadWorktree");
      return Promise.resolve(STATE);
    },
    refreshWorktrees: () => log.push("refreshWorktrees"),
    afterMutation: (state) => {
      log.push("afterMutation");
      states.push(state);
      return Promise.resolve();
    },
    refreshDiff: note("refreshDiff"),
    reselectCommit: () => log.push("reselectCommit"),
    loadGraph: note("loadGraph"),
    freshened: () => {},
  };
  return { log, states, context: { ...base, ...over } };
}

const kinds = (...list: ChangeKind[]) => new Set(list);

describe("answering watcher events", () => {
  it("reads the refs once, without reopening, and the status once, for a commit made elsewhere", async () => {
    const t = context();

    await runDiskPass(t.context, kinds("refs", "head", "index", "workingTree"));

    expect(t.log.filter((name) => name === "refreshRefs")).toHaveLength(1);
    expect(t.log.filter((name) => name === "afterMutation")).toHaveLength(1);
    expect(t.log.filter((name) => name === "loadWorktree")).toHaveLength(1);
  });

  // The status was walked twice per pass: once for the list, once for the counters.
  it("hands the counters of the file list to the after-step instead of walking the status again", async () => {
    const t = context();

    await runDiskPass(t.context, kinds("workingTree"));

    expect(t.states).toEqual([STATE]);
  });

  it("leaves the counters to be read when no file list was", async () => {
    const t = context({ commitSelected: () => true });

    await runDiskPass(t.context, kinds("workingTree"));

    expect(t.states).toEqual([null]);
  });

  it("starts the graph as soon as the refs are in, before the file list is read", async () => {
    let release: () => void = () => {};
    const files = new Promise<typeof STATE>((resolve) => (release = () => resolve(STATE)));
    const t = context();
    const loadWorktree = () => {
      t.log.push("loadWorktree");
      return files;
    };

    const pass = runDiskPass({ ...t.context, loadWorktree }, kinds("refs", "workingTree"));
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(t.log).toContain("loadGraph");
    expect(t.log).not.toContain("afterMutation");
    release();
    await pass;
  });

  it("does not touch the refs or the graph for a plain edit", async () => {
    const t = context();

    await runDiskPass(t.context, kinds("workingTree"));

    expect(t.log).not.toContain("refreshRefs");
    expect(t.log).not.toContain("loadGraph");
    expect(t.log).toContain("afterMutation");
  });
});
