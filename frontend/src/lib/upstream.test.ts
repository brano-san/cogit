import { describe, expect, it } from "vitest";
import { initialUpstream, upstreamProblem } from "./upstream";

const choices = ["origin/main", "origin/topic", "upstream/topic"];

describe("initialUpstream (F-130)", () => {
  it("starts on the branch's current upstream", () => {
    expect(initialUpstream("topic", "upstream/topic", choices, "origin")).toBe("upstream/topic");
  });

  it("otherwise on the branch of the same name, on the primary remote first", () => {
    expect(initialUpstream("topic", null, choices, "origin")).toBe("origin/topic");
    expect(initialUpstream("topic", null, choices, "upstream")).toBe("upstream/topic");
    expect(initialUpstream("topic", null, ["upstream/topic"], "origin")).toBe("upstream/topic");
  });

  it("falls back to the first remote branch, or to none", () => {
    expect(initialUpstream("solo", null, choices, "origin")).toBe("origin/main");
    expect(initialUpstream("solo", null, [], "origin")).toBeNull();
  });

  it("does not start on an upstream the remote no longer has", () => {
    expect(initialUpstream("topic", "origin/gone", choices, "origin")).toBe("origin/topic");
  });
});

describe("upstreamProblem", () => {
  it("needs a remote branch to choose", () => {
    expect(upstreamProblem(null, null, [])).toMatch(/no remote branches/i);
  });

  it("has nothing to do when the choice already is the upstream", () => {
    expect(upstreamProblem("origin/topic", "origin/topic", choices)).toMatch(/already/i);
  });

  it("accepts any other remote branch", () => {
    expect(upstreamProblem("origin/topic", null, choices)).toBeNull();
    expect(upstreamProblem("origin/main", "origin/topic", choices)).toBeNull();
  });
});
