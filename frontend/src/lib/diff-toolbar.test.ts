import { describe, expect, it } from "vitest";
import type { EolInfo, Hunk } from "./ipc/bindings";
import { eolLabel, layoutTip } from "./diff-toolbar";

function eol(old: EolInfo["old"], next: EolInfo["new"]): EolInfo {
  return { old, new: next, normalized: old !== next };
}

function hunk(oldStart: number, oldLines: number, newStart: number, newLines: number): Hunk {
  return { oldStart, oldLines, newStart, newLines, header: "", rows: [] };
}

describe("eolLabel (#17)", () => {
  it("names both sides in capitals", () => {
    expect(eolLabel(eol("lf", "lf"), [hunk(3, 4, 3, 5)])).toEqual({
      text: "LF → LF",
      title: "Line endings: LF → LF",
    });
    expect(eolLabel(eol("crlf", "lf"), [hunk(3, 4, 3, 5)]).text).toBe("CRLF → LF");
  });

  it("shows only the new ending for a new file", () => {
    expect(eolLabel(eol("none", "lf"), [hunk(0, 0, 1, 12)])).toEqual({
      text: "LF",
      title: "Line endings: LF",
    });
  });

  it("shows only the old ending for a deleted file", () => {
    expect(eolLabel(eol("crlf", "none"), [hunk(1, 12, 0, 0)])).toEqual({
      text: "CRLF",
      title: "Line endings: CRLF",
    });
  });

  it("spells out the rarer endings", () => {
    expect(eolLabel(eol("mixed", "cr"), [hunk(1, 2, 1, 2)]).text).toBe("Mixed → CR");
    expect(eolLabel(eol("none", "lf"), [hunk(1, 1, 1, 2)]).text).toBe("None → LF");
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
