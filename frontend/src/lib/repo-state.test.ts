import { describe, expect, it } from "vitest";
import { bannerQuestion, repoStateTag, stateBanner, workingTreeLabel } from "./repo-state";

describe("stateBanner", () => {
  it("shows nothing for a clean repository", () => {
    expect(stateBanner({ kind: "clean" }, null)).toBeNull();
  });

  it("offers only abort for an interrupted merge: the merge commit is an ordinary commit", () => {
    const banner = stateBanner({ kind: "merging" }, null);
    expect(banner?.title).toContain("Merge");
    expect(banner?.actions).toEqual(["abort"]);
    expect(banner?.detail).toContain("commit");
  });

  it("keeps continue where git goes on by itself after the conflicts", () => {
    for (const kind of ["rebasing", "cherryPicking", "reverting", "applyingPatches"] as const) {
      expect(stateBanner({ kind }, null)?.actions, kind).toContain("continue");
    }
  });

  it("offers skip only where a step can be skipped", () => {
    expect(stateBanner({ kind: "rebasing" }, null)?.actions).toEqual([
      "continue",
      "skip",
      "abort",
    ]);
    expect(stateBanner({ kind: "merging" }, null)?.actions).not.toContain("skip");
  });

  it("names the operation so the user knows which one is stuck", () => {
    expect(stateBanner({ kind: "rebasing" }, null)?.title).toContain("Rebase");
    expect(stateBanner({ kind: "cherryPicking" }, null)?.title).toContain("Cherry-pick");
    expect(stateBanner({ kind: "reverting" }, null)?.title).toContain("Revert");
  });

  it("shows the commit for a detached HEAD and offers a branch", () => {
    const banner = stateBanner({ kind: "detachedHead", oid: "4ec48139aa" }, null);
    expect(banner?.detail).toContain("4ec4813");
    expect(banner?.actions).toEqual(["createBranch"]);
  });

  it("warns about an empty repository without offering a way out", () => {
    const banner = stateBanner({ kind: "empty" }, null);
    expect(banner?.severity).toBe("info");
    expect(banner?.actions).toEqual([]);
  });

  it("reports a stale lock above everything else", () => {
    const banner = stateBanner({ kind: "merging" }, "C:/repo/.git/index.lock");
    expect(banner?.title).toContain("locked");
    expect(banner?.severity).toBe("error");
  });

  it("puts the lock path in the detail so it can be deleted by hand", () => {
    expect(stateBanner({ kind: "clean" }, "C:/repo/.git/index.lock")?.detail).toContain(
      "index.lock",
    );
  });

  it("marks an interrupted operation as needing attention", () => {
    expect(stateBanner({ kind: "merging" }, null)?.severity).toBe("warning");
  });
});

describe("a detached HEAD inside a submodule", () => {
  const detached = { kind: "detachedHead", oid: "452e8002c0ffee" } as never;

  it("still warns in a repository the user checked out themselves", () => {
    const shown = stateBanner(detached, null, false);
    expect(shown?.title).toBe("Detached HEAD");
    expect(shown?.detail).toContain("easy to lose");
  });

  // A submodule is detached because its parent records one commit. That is how submodules
  // work, and "commits are easy to lose" is a warning about nothing.
  it("says what it is without the warning when the repository is a submodule", () => {
    const shown = stateBanner(detached, null, true);
    expect(shown?.severity).toBe("info");
    expect(shown?.detail).not.toContain("easy to lose");
    expect(shown?.detail).toContain("452e800");
  });

  it("offers no branch to create for a submodule, which does not want one", () => {
    expect(stateBanner(detached, null, true)?.actions).toEqual([]);
  });

  it("leaves every other state alone whether it is a submodule or not", () => {
    const merging = { kind: "merging" } as never;
    expect(stateBanner(merging, null, true)).toEqual(stateBanner(merging, null, false));
  });
});

describe("git am and bisect", () => {
  it("names git am stopped on a patch and lets it continue, skip or abort", () => {
    const banner = stateBanner({ kind: "applyingPatches" }, null);
    expect(banner?.title).toContain("patches");
    expect(banner?.actions).toEqual(["continue", "skip", "abort"]);
  });

  // git bisect has no --continue and no --skip: it goes on with good, bad or skip.
  it("offers only abort for a bisect", () => {
    expect(stateBanner({ kind: "bisecting" }, null)?.actions).toEqual(["abort"]);
  });
});

describe("workingTreeLabel", () => {
  const status = { staged: 1, unstaged: 2, untracked: 0, conflicted: 3 };

  it("counts what the working tree holds", () => {
    expect(workingTreeLabel(status, { kind: "clean" })).toBe(
      "Working Tree (1 staged, 2 modified, 3 conflicted)",
    );
  });

  it("adds the operation in progress after the counts", () => {
    expect(workingTreeLabel(status, { kind: "merging" })).toBe(
      "Working Tree (1 staged, 2 modified, 3 conflicted), merging",
    );
    expect(workingTreeLabel(undefined, { kind: "applyingPatches" })).toBe(
      "Working Tree, applying patches",
    );
  });

  it("says clean when there is nothing to count", () => {
    const none = { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 };
    expect(workingTreeLabel(none, { kind: "clean" })).toBe("Working Tree — clean");
    expect(workingTreeLabel(none, { kind: "rebasing" })).toBe("Working Tree — clean, rebasing");
  });

  it("leaves a detached HEAD to the banner", () => {
    expect(workingTreeLabel(undefined, { kind: "detachedHead", oid: "abc" })).toBe("Working Tree");
  });
});

describe("repoStateTag", () => {
  it("labels every long-running state in angle brackets", () => {
    const tags = (
      ["merging", "rebasing", "cherryPicking", "reverting", "bisecting", "applyingPatches"] as const
    ).map((kind) => repoStateTag({ kind }));
    expect(tags).toEqual([
      "<merging>",
      "<rebasing>",
      "<cherry-picking>",
      "<reverting>",
      "<bisecting>",
      "<applying patches>",
    ]);
  });

  it("labels a detached HEAD in a repository but not in a submodule, where it is normal", () => {
    const detached = { kind: "detachedHead", oid: "abc" } as const;
    expect(repoStateTag(detached)).toBe("<detached>");
    expect(repoStateTag(detached, true)).toBeNull();
  });

  it("labels nothing when nothing is going on", () => {
    expect(repoStateTag({ kind: "clean" })).toBeNull();
    expect(repoStateTag({ kind: "empty" })).toBeNull();
    expect(repoStateTag(null)).toBeNull();
  });
});

// Abort and Skip ran on the click, one button away from Continue: five resolved conflicts
// gone, and the journal said "cannot be undone".
describe("bannerQuestion", () => {
  const rebasing = { kind: "rebasing" } as never;

  it("asks before Abort and Skip, which Undo cannot take back", () => {
    expect(bannerQuestion("abort", rebasing)).toMatchObject({ confirm: "Abort", warning: true });
    expect(bannerQuestion("abort", rebasing)?.message).toContain("Rebase");
    expect(bannerQuestion("skip", rebasing)).toMatchObject({ confirm: "Skip", warning: true });
  });

  it("lets Continue and Create Branch run at once", () => {
    expect(bannerQuestion("continue", rebasing)).toBeNull();
    expect(bannerQuestion("createBranch", rebasing)).toBeNull();
  });
});
