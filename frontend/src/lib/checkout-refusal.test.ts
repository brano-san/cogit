import { describe, expect, it } from "vitest";
import { blockedByLocalChanges } from "./checkout-refusal";

const OVERWRITE = `error: Your local changes to the following files would be overwritten by checkout:
\tsrc/main.rs
\tREADME.md
Please commit your changes or stash them before you switch branches.
Aborting`;

const UNTRACKED = `error: The following untracked working tree files would be overwritten by checkout:
\tdist/app.js
Please move or remove them before you switch branches.
Aborting`;

describe("blockedByLocalChanges", () => {
  it("recognises the refusal and lists the files git named", () => {
    expect(blockedByLocalChanges(OVERWRITE)).toEqual(["src/main.rs", "README.md"]);
  });

  it("recognises the untracked variant too", () => {
    expect(blockedByLocalChanges(UNTRACKED)).toEqual(["dist/app.js"]);
  });

  it("is null for an unrelated failure, so nothing is offered wrongly", () => {
    expect(blockedByLocalChanges("fatal: invalid reference: nope")).toBeNull();
  });

  it("is null for an empty stderr", () => {
    expect(blockedByLocalChanges("")).toBeNull();
  });

  it("stops at the sentence that follows the list", () => {
    const found = blockedByLocalChanges(OVERWRITE);
    expect(found).not.toContain("Aborting");
    expect(found).toHaveLength(2);
  });

  it("survives a refusal whose file list is empty", () => {
    const bare = "error: Your local changes to the following files would be overwritten by checkout:\nAborting";
    expect(blockedByLocalChanges(bare)).toEqual([]);
  });

  it("accepts spaces as the indent, not only tabs", () => {
    const spaced = OVERWRITE.replace(/\t/g, "    ");
    expect(blockedByLocalChanges(spaced)).toEqual(["src/main.rs", "README.md"]);
  });
});
