import { describe, expect, it } from "vitest";
import { undoRewriteQuestion } from "./undo-rewrite";

const orig = "a".repeat(40);
const head = "b".repeat(40);

describe("undoRewriteQuestion", () => {
  it("refuses with a reason when ORIG_HEAD is missing", () => {
    const q = undoRewriteQuestion(null);
    expect("refused" in q && q.refused).toContain("ORIG_HEAD");
  });

  it("refuses when ORIG_HEAD is HEAD", () => {
    expect("refused" in undoRewriteQuestion({ orig, head: orig, dirty: false })).toBe(true);
  });

  it("names both commits and stays calm on a clean tree", () => {
    const q = undoRewriteQuestion({ orig, head, dirty: false });
    expect("message" in q && q.message).toContain("bbbbbbb to aaaaaaa");
    expect("warning" in q && q.warning).toBe(false);
  });

  it("warns about uncommitted changes", () => {
    const q = undoRewriteQuestion({ orig, head, dirty: true });
    expect("message" in q && q.message).toContain("uncommitted changes");
    expect("warning" in q && q.warning).toBe(true);
  });
});
