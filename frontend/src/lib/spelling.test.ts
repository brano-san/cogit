import { describe, expect, it } from "vitest";
import { accepted, knownWords, mirrorHtml, spellLanguages, spellMenu, spellWords } from "./spelling";

const words = (text: string) => spellWords(text).map((found) => found.word);

describe("spellWords", () => {
  it("takes the words of prose with their offsets", () => {
    expect(spellWords("Fix teh bug.")).toEqual([
      { start: 0, end: 3, word: "Fix" },
      { start: 4, end: 7, word: "teh" },
      { start: 8, end: 11, word: "bug" },
    ]);
  });

  it("checks Russian and apostrophes", () => {
    expect(words("Исправлен «превет», doesn't it’s")).toEqual(["Исправлен", "превет", "doesn't", "it’s"]);
  });

  it("splits hyphenated words into parts", () => {
    expect(words("well-known")).toEqual(["well", "known"]);
  });

  it("leaves code, paths, versions and names in capitals alone", () => {
    expect(
      words("fix(m6): use spellMenu in src/lib/a.ts, v0.4.0, snake_case, README, IPC, https://x.io/a, x"),
    ).toEqual(["use", "in"]);
  });

  it("skips text in backticks, comment lines and trailers", () => {
    const text = "Use `recieve` here\n```\nteh code\n```\n# Plase enter a mesage\nSigned-off-by: Ann Smiht <a@b.c>";
    expect(words(text)).toEqual(["Use", "here"]);
  });
});

describe("mirrorHtml", () => {
  it("marks the misspelled words and escapes the rest", () => {
    const text = "a <b> teh & teh";
    expect(mirrorHtml(text, spellWords(text), new Set(["teh"]))).toBe(
      'a &lt;b&gt; <span class="miss" data-at="6">teh</span> &amp; <span class="miss" data-at="12">teh</span>\n',
    );
  });
});

describe("spellLanguages", () => {
  it("lists the ticked dictionaries", () => {
    expect(spellLanguages({ spellEnUs: true, spellRuRu: true })).toEqual(["en_US", "ru_RU"]);
    expect(spellLanguages({ spellEnUs: false, spellRuRu: true })).toEqual(["ru_RU"]);
    expect(spellLanguages({ spellEnUs: false, spellRuRu: false })).toEqual([]);
  });
});

describe("spellMenu", () => {
  it("offers the suggestions, then adding the word", () => {
    const menu = spellMenu("commit", "teh", ["the", "tech"]);
    expect(menu.map((row) => [row.id, row.label, row.enabled])).toEqual([
      ["child:commit:spell-use-0", "the", true],
      ["child:commit:spell-use-1", "tech", true],
      ["", "", false],
      ["child:commit:spell-add", "Add \"teh\" to Dictionary", true],
    ]);
  });

  it("uses the bare actions in the main window", () => {
    expect(spellMenu(null, "teh", ["the"]).map((row) => row.id)).toEqual(["spell-use-0", "", "spell-add"]);
  });

  it("says when there is nothing to suggest", () => {
    const menu = spellMenu("commit", "qqq", []);
    expect(menu.map((row) => [row.label, row.enabled])).toEqual([
      ["No Suggestions", false],
      ["", false],
      ["Add \"qqq\" to Dictionary", true],
    ]);
  });
});

describe("knownWords", () => {
  it("keeps the stored words, once each, and nothing else", () => {
    expect(knownWords(["Cogit", " gix ", "", 3, "Cogit"])).toEqual(["Cogit", "gix"]);
    expect(knownWords("Cogit")).toBeUndefined();
  });
});

describe("accepted", () => {
  it("matches a dictionary word whatever its case", () => {
    expect(accepted("Cogit", ["cogit"])).toBe(true);
    expect(accepted("gix", ["Cogit"])).toBe(false);
  });
});
