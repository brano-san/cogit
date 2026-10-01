import { describe, expect, it } from "vitest";
import { parseSolver } from "./solver-params";

describe("parseSolver", () => {
  it("reads the repository, the path and the external-tool request", () => {
    expect(parseSolver("?repo=7&path=deep/nested/file.rs")).toEqual({
      repo: 7,
      path: "deep/nested/file.rs",
      tool: false,
    });
    expect(parseSolver("?repo=7&path=a%20b%26c.txt&tool=1")).toEqual({ repo: 7, path: "a b&c.txt", tool: true });
  });

  it("is null without a usable repository or path", () => {
    expect(parseSolver("?repo=1")).toBeNull();
    expect(parseSolver("?path=a.txt")).toBeNull();
    expect(parseSolver("?repo=abc&path=a.txt")).toBeNull();
    expect(parseSolver("?repo=0&path=a.txt")).toBeNull();
  });
});
