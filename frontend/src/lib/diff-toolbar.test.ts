import { describe, expect, it } from "vitest";
import type { EolInfo } from "./ipc/bindings";
import { eolLabel, layoutTip } from "./diff-toolbar";

function eol(old: EolInfo["old"], next: EolInfo["new"]): EolInfo {
  return { old, new: next, normalized: old !== next };
}

describe("eolLabel (#17)", () => {
  it("names both sides in capitals", () => {
    expect(eolLabel(eol("lf", "lf"), 10, 11)).toEqual({
      text: "LF → LF",
      title: "Line endings: LF → LF",
    });
    expect(eolLabel(eol("crlf", "lf"), 10, 11).text).toBe("CRLF → LF");
  });

  it("shows only the new ending for a new file", () => {
    expect(eolLabel(eol("none", "lf"), 0, 12)).toEqual({
      text: "LF",
      title: "Line endings: LF",
    });
  });

  it("shows only the old ending for a deleted file", () => {
    expect(eolLabel(eol("crlf", "none"), 12, 0)).toEqual({
      text: "CRLF",
      title: "Line endings: CRLF",
    });
  });

  it("spells out the rarer endings", () => {
    expect(eolLabel(eol("mixed", "cr"), 2, 2).text).toBe("Mixed → CR");
    expect(eolLabel(eol("none", "lf"), 1, 2).text).toBe("None → LF");
  });

  // DF-006: without context a pure insertion's hunk has no old lines, and the file is
  // still there on both sides.
  it("names both endings when a hunk has an empty side in the middle of the file", () => {
    expect(eolLabel(eol("crlf", "lf"), 10, 12).text).toBe("CRLF → LF");
  });
});

describe("layoutTip (#14)", () => {
  it("says what the button does, then that the choice is kept", () => {
    expect(layoutTip("split")).toBe(
      "Switch to the unified view: both versions in one column (Ctrl+Shift+D)\nRemembered between runs",
    );
    expect(layoutTip("unified")).toBe(
      "Switch to the side-by-side view: old and new versions in two columns (Ctrl+Shift+D)\nRemembered between runs",
    );
  });
});
