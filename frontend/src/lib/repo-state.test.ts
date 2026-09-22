import { describe, expect, it } from "vitest";
import { stateBanner } from "./repo-state";

describe("stateBanner", () => {
  it("shows nothing for a clean repository", () => {
    expect(stateBanner({ kind: "clean" }, null)).toBeNull();
  });

  it("offers continue and abort for an interrupted merge", () => {
    const banner = stateBanner({ kind: "merging" }, null);
    expect(banner?.title).toContain("Merge");
    expect(banner?.actions).toEqual(["continue", "abort"]);
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
