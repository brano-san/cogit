import { describe, expect, it } from "vitest";
import { tokenizeIgnoreLine as tokens } from "./ignore-syntax";

const classes = (line: string) =>
  tokens(line)
    .filter((token) => token.cls !== "")
    .map((token) => [token.cls, token.text]);

describe("tokenizeIgnoreLine", () => {
  it("gives back exactly the line it was given", () => {
    for (const line of ["", "# note", "!keep.log", "build/", "**/*.tmp", "a\\#b", "[Dd]ebug/", "  spaced  ", "[open"]) {
      expect(tokens(line).map((token) => token.text).join("")).toBe(line);
    }
  });

  it("colors comments, negation, wildcards, classes and escapes", () => {
    expect(classes("# note")).toEqual([["tok-comment", "# note"]]);
    expect(classes("!keep.log")).toEqual([["tok-keyword", "!"]]);
    expect(classes("**/*.tmp")).toEqual([
      ["tok-atom", "**"],
      ["tok-atom", "*"],
    ]);
    expect(classes("[Dd]ebug/")).toEqual([["tok-string", "[Dd]"]]);
    expect(classes("\\#file")).toEqual([["tok-number", "\\#"]]);
  });
});
