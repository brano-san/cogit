import { describe, expect, it } from "vitest";
import type { OriginCandidate, OriginReport } from "$lib/ipc/investigate";
import { PERSPECTIVES, panelsOf, perspectiveAfterSearch, searchesOrigins } from "./perspectives";

function report(kind: OriginCandidate["kind"]): OriginReport {
  const candidate: OriginCandidate = {
    kind,
    rev: "p",
    path: "a.rs",
    from: 1,
    to: 2,
    score: 90,
    likelihood: "high",
    deeper: null,
    block: [],
    source: [],
  };
  return { candidates: [candidate], best: 0 };
}

describe("Investigate perspectives", () => {
  it("offers DeepGit's five, each with its own shortcut", () => {
    expect(PERSPECTIVES.map((p) => p.label)).toEqual([
      "Log",
      "Diff",
      "Blame",
      "Blame+Origins",
      "Origins",
    ]);
    expect(new Set(PERSPECTIVES.map((p) => p.shortcut)).size).toBe(5);
  });

  it("shows the candidates beside Blame only in Blame+Origins", () => {
    expect(panelsOf("blame")).toMatchObject({ blame: true, candidates: false });
    expect(panelsOf("blameOrigins")).toMatchObject({ blame: true, candidates: true });
    expect(panelsOf("origins")).toMatchObject({ blame: false, candidates: true, origin: true });
    expect(panelsOf("log")).toMatchObject({ log: true, blame: false, diff: false });
  });

  it("searches origins wherever lines can be picked", () => {
    expect(searchesOrigins("blame")).toBe(true);
    expect(searchesOrigins("origins")).toBe(true);
    expect(searchesOrigins("diff")).toBe(false);
  });

  it("switches plain Blame to Blame+Origins when the lines came from elsewhere", () => {
    expect(perspectiveAfterSearch("blame", report("moved"))).toBe("blameOrigins");
    expect(perspectiveAfterSearch("blame", report("copied"))).toBe("blameOrigins");
    expect(perspectiveAfterSearch("blame", report("appeared"))).toBe("blame");
    expect(perspectiveAfterSearch("origins", report("moved"))).toBe("origins");
    expect(perspectiveAfterSearch("blame", null)).toBe("blame");
  });
});
