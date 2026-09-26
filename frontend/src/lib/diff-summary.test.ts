import { describe, expect, it } from "vitest";
import { binaryReason, byteCount, summaryRows, tooLargeReason } from "./diff-summary";

describe("diff summary", () => {
  it("counts bytes exactly, with separators", () => {
    expect(byteCount(1_000_000)).toBe("1,000,000 bytes");
    expect(byteCount(1)).toBe("1 byte");
    expect(byteCount(0)).toBe("0 bytes");
  });

  it("gives SmartGit's reason for a character text does not hold", () => {
    expect(binaryReason({ kind: "character", code: 2, line: 1, position: 6, side: "new" })).toBe(
      "File is considered as binary: invalid character 0x02 in line 1, at position 6 (new version)",
    );
  });

  it("names the attribute that makes a file binary", () => {
    expect(binaryReason({ kind: "attribute", name: "-diff" })).toBe(
      "File is considered as binary: .gitattributes marks it -diff",
    );
  });

  it("says the limit a large file is past", () => {
    expect(tooLargeReason(1_000_000)).toBe("File size exceeds the limit of 1,000,000 bytes");
  });

  it("says a deleted file has no new version rather than 0 bytes", () => {
    const [old, next] = summaryRows({ size: 12, id: "a".repeat(40) }, null);

    expect(old).toEqual({ label: "Old version", size: "12 bytes", id: "a".repeat(40), note: null });
    expect(next?.size).toBe("Not there: the file is deleted");
    expect(next?.id).toBeNull();
  });

  it("says why a present side has no id", () => {
    const [, next] = summaryRows({ size: 1, id: null }, { size: 30_000_000, id: null });

    expect(next?.note).toMatch(/too large to read/);
  });
});
