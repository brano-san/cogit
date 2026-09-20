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
