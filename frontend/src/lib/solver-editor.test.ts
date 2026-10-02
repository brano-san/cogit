import { describe, expect, it } from "vitest";
import { themeSpec } from "./solver-editor";
import { docsFromRegions } from "./solver-model";

describe("Conflict Solver panes", () => {
  it("opts the editable text back into selection under the body's user-select: none", () => {
    expect(themeSpec[".cm-content"].userSelect).toBe("text");
    expect(themeSpec[".cm-content"].WebkitUserSelect).toBe("text");
  });

  it("every pane document holds the text of the regions", () => {
    const docs = docsFromRegions([
      { kind: "equal", base: ["a"], ours: ["a"], theirs: ["a"], result: ["a"] },
      { kind: "conflict", base: ["b"], ours: ["b1"], theirs: ["b2"], result: ["b"] },
    ]);
    expect(docs.oursText).toBe("a\nb1\n");
    expect(docs.theirsText).toBe("a\nb2\n");
    expect(docs.resultText).toBe("a\nb\n");
  });
});
