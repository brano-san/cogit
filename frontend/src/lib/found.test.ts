import { describe, expect, it } from "vitest";
import type { Found } from "./ipc";
import { foundStep } from "./found";

const found = (kind: Found["kind"], label: string, oid = "c1"): Found => ({ kind, label, detail: "", oid });

// Find Object checked a picked branch out without asking, and a remote one failed with
// "a branch is expected, got remote branch"; T8.2 asks for the object shown in the panel.
describe("picking a result in Find Object", () => {
  it("goes to a branch's tip, local or remote, and checks nothing out", () => {
    expect(foundStep(found("branch", "feature/x", "b1"), false)).toEqual({ kind: "reveal", oid: "b1" });
    expect(foundStep(found("branch", "origin/feature/x", "b2"), false)).toEqual({ kind: "reveal", oid: "b2" });
  });

  it("goes to a tag's commit and to a commit", () => {
    expect(foundStep(found("tag", "v1", "t1"), false)).toEqual({ kind: "reveal", oid: "t1" });
    expect(foundStep(found("commit", "c9", "c9"), false)).toEqual({ kind: "reveal", oid: "c9" });
  });

  it("opens a file of the selected commit", () => {
    expect(foundStep(found("file", "src/a.rs", ""), true)).toEqual({ kind: "file", path: "src/a.rs" });
  });

  // On the Working Tree a picked file did nothing at all, and the window had closed already.
  it("opens a file on the Working Tree in the list that has it", () => {
    const staged = (path: string) => path === "src/b.rs";
    expect(foundStep(found("file", "src/a.rs", ""), false, staged)).toEqual({ kind: "worktree", path: "src/a.rs" });
    expect(foundStep(found("file", "src/b.rs", ""), false, staged)).toEqual({ kind: "staged", path: "src/b.rs" });
  });

  it("does nothing for a ref that points at no commit", () => {
    expect(foundStep(found("tag", "v0", ""), false)).toBeNull();
  });
});
