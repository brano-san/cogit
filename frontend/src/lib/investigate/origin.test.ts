import { describe, expect, it } from "vitest";
import type { OriginCandidate, OriginReport } from "$lib/ipc/investigate";
import { deeperHint, describeCandidate, likelihoodLabel } from "./origin";

function candidate(
  kind: OriginCandidate["kind"],
  path = "src/a.rs",
  likelihood: OriginCandidate["likelihood"] = "high",
): OriginCandidate {
  return {
    kind,
    rev: "p",
    path,
    from: 12,
    to: 20,
    score: 90,
    likelihood,
    deeper: { rev: "p", path, line: 14 },
    block: [],
    source: [],
  };
}

describe("origin wording", () => {
  it("says the lines appeared here, as DeepGit's card does", () => {
    expect(describeCandidate(candidate("appeared"), "src/a.rs")).toEqual({
      title: "Appeared here",
      detail: "Lines first appeared at this position",
    });
  });

  it("tells a move within the file from a move out of another one", () => {
    expect(describeCandidate(candidate("moved"), "src/a.rs").title).toBe("Moved within the file");
    expect(describeCandidate(candidate("moved", "lib/b.rs"), "src/a.rs")).toEqual({
      title: "Moved from b.rs",
      detail: "Removed from lib/b.rs by the same commit",
    });
    expect(describeCandidate(candidate("copied", "lib/b.rs"), "src/a.rs").title).toBe("Copied from b.rs");
  });

  it("labels a lone candidate a single origin", () => {
    const report: OriginReport = { candidates: [candidate("appeared")], best: 0 };
    expect(likelihoodLabel(report, 0)).toBe("single origin, high likelihood");
  });

  it("ranks candidates when there are rivals", () => {
    const report: OriginReport = {
      candidates: [candidate("appeared", "src/a.rs", "low"), candidate("moved", "b.rs")],
      best: 1,
    };
    expect(likelihoodLabel(report, 1)).toBe("best of 2 origins, high likelihood");
    expect(likelihoodLabel(report, 0)).toBe("1 of 2 origins, low likelihood");
    expect(likelihoodLabel(report, 5)).toBe("");
  });

  it("explains where Go Deeper leads, or why it cannot", () => {
    expect(deeperHint(candidate("moved", "lib/b.rs"))).toBe(
      "Blame b.rs as it was before, at line 14",
    );
    expect(deeperHint({ ...candidate("appeared"), deeper: null })).toMatch(/^Nothing older/);
    expect(deeperHint(null)).toBe("Pick a line first");
  });
});
