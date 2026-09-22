import { describe, expect, it } from "vitest";
import { tokenizeConfigLine as tokens } from "./config-syntax";

const classes = (line: string) =>
  tokens(line)
    .filter((token) => token.cls !== "")
    .map((token) => [token.cls, token.text.trim()]);

describe("tokenizeConfigLine", () => {
  // The paint layer sits under the textarea; a token lost or added moves every caret.
  it("gives back exactly the line it was given", () => {
    for (const line of [
      "",
      "[core]",
      '[remote "origin"]',
      "\tbare = false",
      "  url = https://example.com/a.git # inline",
      "; a comment",
      '\tname = "a # not a comment"',
      "\tflag",
      "  [broken",
    ]) {
      expect(tokens(line).map((token) => token.text).join("")).toBe(line);
    }
  });

  it("colours a section and its subsection", () => {
    expect(classes('[remote "origin"]')).toEqual([
      ["tok-typeName", "[remote"],
      ["tok-string", '"origin"'],
      ["tok-typeName", "]"],
    ]);
  });

  it("colours a key and its value", () => {
    expect(classes("\turl = https://example.com/a.git")).toEqual([
      ["tok-propertyName", "url"],
      ["tok-string", "https://example.com/a.git"],
    ]);
  });

  it("colours a boolean and a number as what they are", () => {
    expect(classes("\tbare = false")).toContainEqual(["tok-bool", "false"]);
    expect(classes("\tdepth = 50")).toContainEqual(["tok-number", "50"]);
  });

  it("treats a line starting with # or ; as a comment", () => {
    expect(classes("# a comment")).toEqual([["tok-comment", "# a comment"]]);
    expect(classes("  ; another")).toEqual([["tok-comment", "; another"]]);
  });

  it("splits off a comment after a value", () => {
    expect(classes("\turl = x # why")).toEqual([
      ["tok-propertyName", "url"],
      ["tok-string", "x"],
      ["tok-comment", "# why"],
    ]);
  });

  it("does not take a # inside quotes for a comment", () => {
    expect(classes('\tname = "a # b"')).toEqual([
      ["tok-propertyName", "name"],
      ["tok-string", '"a # b"'],
    ]);
  });

  it("colours a key with no value, which git reads as true", () => {
    expect(classes("\tflag")).toEqual([["tok-propertyName", "flag"]]);
  });
});
