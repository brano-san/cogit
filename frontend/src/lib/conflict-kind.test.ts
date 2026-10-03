import { describe, expect, it } from "vitest";
import {
  externalToolRefusal,
  sideTitle,
  submoduleCommit,
  wholeReason,
} from "./conflict-kind";

describe("what a whole-file conflict is called", () => {
  it("names a submodule and a symbolic link, not 'binary'", () => {
    expect(wholeReason("submodule", true)).toContain("submodule");
    expect(wholeReason("symlink", true)).toContain("symbolic link");
    expect(wholeReason("symlink", true)).not.toContain("binary");
    expect(wholeReason("regular", true)).toContain("binary");
    expect(wholeReason("regular", false)).toContain("4 MiB");
  });

  it("titles the sides of a submodule", () => {
    expect(sideTitle("submodule", "ours")).toBe("Submodule: ours");
    expect(sideTitle("submodule", "theirs")).toBe("Submodule: theirs");
    expect(sideTitle("regular", "ours")).toBe("Ours");
    expect(sideTitle("symlink", "base")).toBe("Base");
  });

  it("shortens the commit a side points to", () => {
    const stages = { base: null, ours: "1234567890abcdef", theirs: null };
    expect(submoduleCommit(stages, "ours")).toBe("1234567");
    expect(submoduleCommit(stages, "theirs")).toBeNull();
    expect(submoduleCommit(null, "ours")).toBeNull();
  });

  it("refuses an external tool for a submodule only", () => {
    expect(externalToolRefusal("submodule")).toBeTruthy();
    expect(externalToolRefusal("symlink")).toBeUndefined();
    expect(externalToolRefusal("regular")).toBeUndefined();
  });
});
