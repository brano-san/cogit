import { describe, expect, it } from "vitest";
import { splitLinks } from "./links";

describe("splitLinks", () => {
  it("returns plain text as one piece", () => {
    expect(splitLinks("nothing here")).toEqual([{ text: "nothing here", href: null }]);
  });

  it("finds a bare https url", () => {
    expect(splitLinks("see https://example.com/x now")).toEqual([
      { text: "see ", href: null },
      { text: "https://example.com/x", href: "https://example.com/x" },
      { text: " now", href: null },
    ]);
  });

  it("finds the pull request link git prints on push", () => {
    const line = "remote: Create a pull request at https://github.com/o/r/pull/new/topic";
    const parts = splitLinks(line);
    expect(parts.at(-1)?.href).toBe("https://github.com/o/r/pull/new/topic");
  });

  it("finds several urls in one line", () => {
    expect(splitLinks("a https://one.test b http://two.test").filter((p) => p.href)).toHaveLength(
      2,
    );
  });

  it("does not swallow the trailing punctuation of a sentence", () => {
    const parts = splitLinks("open https://example.com.");
    expect(parts.find((p) => p.href)?.href).toBe("https://example.com");
    expect(parts.at(-1)?.text).toBe(".");
  });

  it("does not treat a bare word as a link", () => {
    expect(splitLinks("httpsomething").every((p) => p.href === null)).toBe(true);
  });

  it("keeps an empty string empty", () => {
    expect(splitLinks("")).toEqual([]);
  });
})
