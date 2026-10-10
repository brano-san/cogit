import { describe, expect, it } from "vitest";
import type { Branch } from "$lib/ipc";
import {
  EMPTY_DEFAULTS,
  chooseLocal,
  fetchOptionsOf,
  mergeDefaults,
  pullChoiceOf,
  pullCommandLine,
  pullOptionsOf,
  pushBranchesOf,
  pushTargetOf,
  pushChoiceOf,
  pushOptionsOf,
  pushProblem,
  remoteBranchChoices,
  type PushChoice,
} from "./network-dialogs";

const push = (over: Partial<PushChoice> = {}): PushChoice => ({
  remote: "origin",
  local: "main",
  branch: "main",
  setUpstream: false,
  tags: "none",
  notes: false,
  forceWithLease: false,
  forceIfIncludes: true,
  noVerify: false,
  dryRun: false,
  atomic: false,
  ...over,
});

describe("pull", () => {
  it("opens on what the repository remembers", () => {
    expect(pullChoiceOf(EMPTY_DEFAULTS)).toEqual({ method: "merge", tags: false, notes: false });
    expect(pullChoiceOf({ ...EMPTY_DEFAULTS, pullMethod: "rebase", pullNotes: true })).toEqual({
      method: "rebase",
      tags: false,
      notes: true,
    });
  });

  it("keeps the fast-forward setting for a merge only", () => {
    const choice = { method: "merge", tags: true, notes: false } as const;
    expect(pullOptionsOf(choice, true)).toEqual({ method: "merge", ffOnly: true, fetch: { tags: true, notes: false }, autostash: false });
    expect(pullOptionsOf({ ...choice, method: "rebase" }, true).ffOnly).toBe(false);
  });

  it("Fetch Only carries the tag and notes options and nothing else", () => {
    expect(fetchOptionsOf({ method: "rebase", tags: true, notes: true })).toEqual({ tags: true, notes: true });
  });

  it("previews the command as the engine builds it", () => {
    expect(pullCommandLine("origin", { method: "merge", tags: true, notes: false }, false)).toBe(
      "git pull --progress --prune --tags --force --no-rebase origin",
    );
    expect(pullCommandLine("o", { method: "rebase", tags: false, notes: true }, true)).toBe(
      "git fetch +refs/notes/*:refs/notes-remote/o/*; git pull --progress --prune --rebase o",
    );
  });
});

describe("push", () => {
  it("sets the upstream only while there is none, whatever was remembered", () => {
    const base = { local: "main", remote: "origin", branch: "main" };
    expect(pushChoiceOf(EMPTY_DEFAULTS, { ...base, hasUpstream: false }).setUpstream).toBe(true);
    expect(pushChoiceOf(EMPTY_DEFAULTS, { ...base, hasUpstream: true }).setUpstream).toBe(false);
    expect(Object.keys(EMPTY_DEFAULTS)).not.toContain("pushSetUpstream");
  });

  it("never opens with force on, whatever was remembered", () => {
    const choice = pushChoiceOf(
      { ...EMPTY_DEFAULTS, pushTags: "follow", pushNotes: true },
      { local: "a", remote: "o", branch: "a", hasUpstream: true },
    );
    expect(choice.forceWithLease).toBe(false);
    expect(choice.forceIfIncludes).toBe(true);
    expect(choice).toMatchObject({ tags: "follow", notes: true });
    expect(choice).toMatchObject({ noVerify: false, dryRun: false, atomic: false });
  });

  it("hands the engine every field and the tip the lease expects", () => {
    const { dryRun: _dryRun, ...sent } = push({ tags: "follow", noVerify: true, atomic: true, dryRun: true });
    expect(pushOptionsOf(push({ tags: "follow", noVerify: true, atomic: true, dryRun: true }), "abc")).toEqual({ ...sent, leaseExpect: "abc" });
    expect(pushOptionsOf(push()).leaseExpect).toBeNull();
  });

  it("refuses an empty or malformed target", () => {
    expect(pushProblem(push())).toBeNull();
    expect(pushProblem(push({ branch: "feat/a-b" }))).toBeNull();
    expect(pushProblem(push({ remote: "" }))).toMatch(/remote/i);
    expect(pushProblem(push({ branch: "" }))).toMatch(/branch/i);
    expect(pushProblem(push({ branch: "a b" }))).toMatch(/branch/i);
    expect(pushProblem(push({ branch: "a..b" }))).toMatch(/branch/i);
  });

  it("offers the remote's branches and always the local name", () => {
    expect(remoteBranchChoices("origin", ["origin/main", "origin/dev", "up/x", "origin/feat/a"], "topic")).toEqual([
      "dev",
      "feat/a",
      "main",
      "topic",
    ]);
    expect(remoteBranchChoices("origin", ["origin/main"], "main")).toEqual(["main"]);
  });

});

describe("remembering", () => {
  it("replaces only the half that was asked to be remembered", () => {
    const stored = { ...EMPTY_DEFAULTS, pushTags: "all", pushNotes: true } as const;
    const merged = mergeDefaults(stored, {
      pull: { method: "rebase", tags: true, notes: false },
    });
    expect(merged).toMatchObject({ pullMethod: "rebase", pullTags: true, pushTags: "all", pushNotes: true });
  });

  it("remembers the push choices but never force", () => {
    const merged = mergeDefaults(EMPTY_DEFAULTS, {
      push: push({ tags: "follow", notes: true, setUpstream: true, forceWithLease: true }),
    });
    expect(merged).toEqual({ ...EMPTY_DEFAULTS, pushTags: "follow", pushNotes: true, pushRemote: "origin" });
  });
});

describe("where a push goes", () => {
  const branch = { name: "topic", upstream: null, pushRemote: null, pushTarget: null };
  const remotes = ["origin", "fork"];

  it("sends a new branch to the primary remote under its own name", () => {
    expect(pushTargetOf(branch, remotes, "origin")).toEqual({ remote: "origin", branch: "topic" });
  });

  it("follows the upstream's name on the same remote", () => {
    expect(pushTargetOf({ ...branch, upstream: "origin/main" }, remotes, "origin")).toEqual({
      remote: "origin",
      branch: "main",
    });
  });

  it("keeps the local name when the push remote is not the upstream's", () => {
    expect(pushTargetOf({ ...branch, upstream: "origin/main", pushRemote: "fork" }, remotes, "origin")).toEqual({
      remote: "fork",
      branch: "topic",
    });
  });

  it("goes where a triangular workflow says", () => {
    expect(pushTargetOf({ ...branch, pushTarget: "fork/feat/x" }, remotes, "origin")).toEqual({
      remote: "fork",
      branch: "feat/x",
    });
  });

  it("has no remote when the repository has none", () => {
    expect(pushTargetOf(branch, [], null).remote).toBeNull();
  });

  it("opens on the remembered remote while it exists", () => {
    expect(pushTargetOf({ ...branch, upstream: "origin/main" }, remotes, "origin", "fork")).toEqual({
      remote: "fork",
      branch: "topic",
    });
    expect(pushTargetOf({ ...branch, upstream: "fork/dev" }, remotes, "origin", "fork")).toEqual({
      remote: "fork",
      branch: "dev",
    });
    expect(pushTargetOf({ ...branch, pushTarget: "fork/feat/x" }, remotes, "origin", "fork")).toEqual({
      remote: "fork",
      branch: "feat/x",
    });
    expect(pushTargetOf({ ...branch, pushTarget: "fork/feat/x" }, remotes, "origin", "origin")).toEqual({
      remote: "origin",
      branch: "topic",
    });
    expect(pushTargetOf(branch, remotes, "origin", "gone")).toEqual({ remote: "origin", branch: "topic" });
  });
});

describe("the branches a push can take", () => {
  const branch = (name: string, kind: "local" | "remote", upstream: string | null = null) =>
    ({ name, kind, upstream, pushRemote: null, pushTarget: null }) as unknown as Branch;

  it("offers every local branch with where it goes, and every remote-tracking name", () => {
    expect(pushBranchesOf([branch("main", "local", "origin/main"), branch("origin/main", "remote"), branch("topic", "local")])).toEqual({
      locals: [
        { name: "main", upstream: "origin/main", pushRemote: null, pushTarget: null },
        { name: "topic", upstream: null, pushRemote: null, pushTarget: null },
      ],
      remoteBranches: ["origin/main"],
    });
  });

  it("moves the target with the local branch", () => {
    const topic = { name: "topic", upstream: "fork/topic", pushRemote: "fork", pushTarget: null };
    expect(chooseLocal(push(), topic, ["origin", "fork"], EMPTY_DEFAULTS)).toMatchObject({
      local: "topic",
      remote: "fork",
      branch: "topic",
      setUpstream: false,
    });
    const fresh = { name: "new", upstream: null, pushRemote: null, pushTarget: null };
    expect(chooseLocal(push({ remote: "fork" }), fresh, ["origin", "fork"], EMPTY_DEFAULTS)).toMatchObject({
      local: "new",
      remote: "fork",
      branch: "new",
      setUpstream: true,
    });
  });
});
