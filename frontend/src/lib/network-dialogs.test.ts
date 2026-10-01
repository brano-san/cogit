import { describe, expect, it } from "vitest";
import {
  EMPTY_DEFAULTS,
  fetchOptionsOf,
  mergeDefaults,
  pullChoiceOf,
  pullCommandLine,
  pullOptionsOf,
  pushTargetOf,
  pushChoiceOf,
  pushCommandLine,
  pushOptionsOf,
  pushProblem,
  remoteBranchChoices,
  tagsHint,
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
    expect(pullOptionsOf(choice, true)).toEqual({ method: "merge", ffOnly: true, fetch: { tags: true, notes: false } });
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
  it("sets the upstream while there is none, unless the repository chose", () => {
    const base = { local: "main", remote: "origin", branch: "main" };
    expect(pushChoiceOf(EMPTY_DEFAULTS, { ...base, hasUpstream: false }).setUpstream).toBe(true);
    expect(pushChoiceOf(EMPTY_DEFAULTS, { ...base, hasUpstream: true }).setUpstream).toBe(false);
    expect(pushChoiceOf({ ...EMPTY_DEFAULTS, pushSetUpstream: false }, { ...base, hasUpstream: false }).setUpstream).toBe(false);
  });

  it("never opens with force on, whatever was remembered", () => {
    const choice = pushChoiceOf(
      { ...EMPTY_DEFAULTS, pushTags: "follow", pushNotes: true },
      { local: "a", remote: "o", branch: "a", hasUpstream: true },
    );
    expect(choice.forceWithLease).toBe(false);
    expect(choice).toMatchObject({ tags: "follow", notes: true });
  });

  it("previews the command", () => {
    expect(pushCommandLine(push())).toBe("git push --progress origin refs/heads/main:refs/heads/main");
    expect(pushCommandLine(push({ setUpstream: true, tags: "all", notes: true, forceWithLease: true }))).toBe(
      "git push --progress --set-upstream --tags --force-with-lease --force-if-includes origin refs/heads/main:refs/heads/main; git push --progress origin refs/notes/*:refs/notes/*",
    );
  });

  it("hands the engine every field", () => {
    expect(pushOptionsOf(push({ tags: "follow" }))).toEqual(push({ tags: "follow" }));
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

  it("wraps the tags choice in words", () => {
    expect(tagsHint("follow")).toMatch(/annotated/i);
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
    expect(merged).toMatchObject({ pushTags: "follow", pushNotes: true, pushSetUpstream: true });
    expect(JSON.stringify(merged)).not.toMatch(/force/i);
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
});
