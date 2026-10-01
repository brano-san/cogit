import { describe, expect, it } from "vitest";
import type { EolInfo } from "./ipc/bindings";
import { alignedButton, bandAction, isAbsentFromBothSides, eolChangeText, eolLabel, layoutTip, modeChangeText, whitespaceButton } from "./diff-toolbar";

function eol(old: EolInfo["old"], next: EolInfo["new"]): EolInfo {
  return { old, new: next, normalized: old !== next };
}

describe("eolLabel (#17)", () => {
  it("warns, and explains, only when both sides exist and the endings differ", () => {
    const changed = eolLabel(eol("lf", "crlf"), 10, 10);
    expect(changed.warn).toBe(true);
    expect(changed.title).toMatch(/line endings of this file change/);
    expect(eolLabel(eol("lf", "lf"), 10, 10).warn).toBe(false);
    expect(eolLabel(eol("none", "crlf"), 0, 10).warn).toBe(false);
  });

  it("names both sides in capitals", () => {
    expect(eolLabel(eol("lf", "lf"), 10, 11)).toEqual({
      text: "LF → LF",
      title: "Line endings: LF → LF",
      warn: false,
    });
    expect(eolLabel(eol("crlf", "lf"), 10, 11).text).toBe("CRLF → LF");
  });

  it("shows only the new ending for a new file", () => {
    expect(eolLabel(eol("none", "lf"), 0, 12)).toEqual({
      text: "LF",
      title: "Line endings: LF",
      warn: false,
    });
  });

  it("shows only the old ending for a deleted file", () => {
    expect(eolLabel(eol("crlf", "none"), 12, 0)).toEqual({
      text: "CRLF",
      title: "Line endings: CRLF",
      warn: false,
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

describe("eolChangeText (DF-028)", () => {
  it("spells the endings as the toolbar does, not as the wire does", () => {
    expect(eolChangeText("crlf", "lf")).toBe("CRLF → LF");
    expect(eolChangeText("mixed", "none")).toBe("Mixed → None");
  });
});

describe("modeChangeText (DF-026)", () => {
  it("names both modes and what the executable bit did", () => {
    expect(modeChangeText("100644", "100755")).toBe("100644 → 100755 (now executable)");
    expect(modeChangeText("100755", "100644")).toBe("100755 → 100644 (no longer executable)");
    expect(modeChangeText("100644", "120000")).toBe("100644 → 120000");
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

describe("whitespaceButton (R-626)", () => {
  it("is pressed only while something is ignored, and marks indentation otherwise", () => {
    expect(whitespaceButton("none")).toMatchObject({ pressed: false, indent: true });
    expect(whitespaceButton("trailing")).toMatchObject({ pressed: true, indent: true });
    expect(whitespaceButton("all")).toMatchObject({ pressed: true, indent: false });
  });

  it("cycles through all modes and names the next one in the tooltip", () => {
    expect(whitespaceButton("none").next).toBe("trailing");
    expect(whitespaceButton("trailing").next).toBe("all");
    expect(whitespaceButton("all").next).toBe("none");
    expect(whitespaceButton("all").title).toMatch(/Click for: Whitespace: shown/);
  });
});

describe("alignedButton", () => {
  it("is pressed while the sides are aligned, and says what is on and what a click does", () => {
    const on = alignedButton("aligned");
    expect(on).toMatchObject({ label: "Aligned", pressed: true, next: "compact" });
    expect(on.title).toContain("ON: Aligned 1:1");
    expect(on.title).toContain("Click for: Compact");
    const off = alignedButton("compact");
    expect(off).toMatchObject({ pressed: false, next: "aligned" });
    expect(off.title).toContain("OFF: Compact");
    expect(off.title).toContain("Click for: Aligned 1:1");
  });
});

describe("bandAction (item 2)", () => {
  const actions = (kind: "workTreeVsIndex" | "indexVsHead" | "commit") => ({
    stage: kind === "workTreeVsIndex",
    unstage: kind === "indexVsHead",
    discard: kind === "workTreeVsIndex",
  });

  it("discards from the working tree against the index", () => {
    expect(bandAction(true, actions("workTreeVsIndex"))).toBe("discard");
  });

  it("offers nothing in the staged diff: Unstage lives on the hunks and in the toolbar", () => {
    expect(bandAction(true, actions("indexVsHead"))).toBeNull();
  });

  it("offers nothing where nothing can be staged", () => {
    expect(bandAction(false, actions("workTreeVsIndex"))).toBeNull();
  });
});

describe("isAbsentFromBothSides", () => {
  it("recognises the engine's message about a file that is gone from both sides", () => {
    const text = "Invalid repository state: doc/a.md is absent from both sides of the diff";
    expect(isAbsentFromBothSides({ message: text })).toBe(true);
  });

  it("does not take any other failure for it", () => {
    expect(isAbsentFromBothSides({ message: "io error: access denied" })).toBe(false);
    expect(isAbsentFromBothSides(null)).toBe(false);
  });
});
