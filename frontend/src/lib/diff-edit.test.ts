import { describe, expect, it } from "vitest";
import type { FileDiff } from "$lib/ipc";
import { editOffer } from "./diff-edit";

const text = { kind: "text", hunks: [], lossyEncoding: false, converted: null } as unknown as FileDiff;

describe("editOffer", () => {
  it("offers Edit only where the right side is the working tree", () => {
    expect(editOffer({ kind: "workTreeVsIndex" }, text)).toEqual({ blocked: null });
    expect(editOffer({ kind: "commitVsWorkTree", oid: "abc" }, text)).toEqual({ blocked: null });
    expect(editOffer({ kind: "indexVsHead" }, text)).toBeNull();
    expect(editOffer({ kind: "commitVsParent", oid: "abc" }, text)).toBeNull();
    expect(editOffer({ kind: "commitVsCommit", a: "abc", b: "def" }, text)).toBeNull();
  });

  it("explains why a binary, large or converted file is not editable", () => {
    const spec = { kind: "workTreeVsIndex" } as const;
    expect(editOffer(spec, { kind: "binary" } as unknown as FileDiff)?.blocked).toMatch(/binary/);
    expect(editOffer(spec, { kind: "tooLarge" } as unknown as FileDiff)?.blocked).toMatch(/too large/);
    expect(editOffer(spec, { ...text, converted: "UTF-16" } as FileDiff)?.blocked).toMatch(/converted/);
  });
});
