import { beforeAll, describe, expect, it } from "vitest";
import { MAX_HIGHLIGHT_LINES, highlightLines, languageOf, loadLanguage, mergePieces } from "./highlight";

function classesOf(tokens: { cls: string }[][], line: number): string[] {
  return (tokens[line] ?? []).map((t) => t.cls);
}

describe("highlightLines", () => {
  it("returns nothing for a language it does not know", () => {
    expect(highlightLines(["fn main() {}"], "klingon")).toEqual([[]]);
  });

  it("returns nothing when no language is given", () => {
    expect(highlightLines(["fn main() {}"], null)).toEqual([[]]);
  });

  const SAMPLES: Record<string, string> = {
    c: "int main() { return 0; }",
    cpp: "class Foo { public: int x; };",
    css: "body { color: red; }",
    html: "<div>test</div>",
    javascript: "const x = 1;",
    json: '{"key": "value"}',
    python: "def foo(): pass",
    rust: "fn main() {}",
    typescript: "const x: number = 1;",
    jsx: "const el = <div>hi</div>;",
    tsx: "const el = <div>hi</div>;",
    java: "public class Main { public static void main(String[] args) {} }",
    yaml: "key: value",
    xml: "<note><to>User</to></note>",
    markdown: "# Header\nSome text",
    php: "<?php echo 'hello'; ?>",
    sass: "$primary-color: #333;\nbody { color: $primary-color; }",
    csharp: "class Program { static void Main() {} }",
    go: "package main\nfunc main() {}",
    ruby: "def hello; end",
    shell: "if [ -f file ]; then echo hi; fi",
    sql: "SELECT id FROM users WHERE id = 1;",
    toml: "name = 'test'",
    kotlin: "fun main() {}",
    swift: "func main() {}",
    cmake: "cmake_minimum_required(VERSION 3.10)",
    dockerfile: "FROM alpine:latest",
    lua: "function test() end",
    perl: "sub test { return 1; }",
    r: "function(x) { x + 1 }",
    scala: "def main(args: Array[String]): Unit = {}",
    dart: "void main() {}",
    haskell: "module Main where",
    groovy: "def list = [1, 2, 3]",
    powershell: "function Test-Cmdlet { return 1 }",
    svelte: "<div>{name}</div>",
    vue: "<template><div>hi</div></template>",
  };

  beforeAll(async () => {
    for (const language of Object.keys(SAMPLES)) expect(await loadLanguage(language), language).toBe(true);
  });

  it("does not load a language it does not know", async () => {
    expect(await loadLanguage("klingon")).toBe(false);
  });

  for (const [language, code] of Object.entries(SAMPLES)) {
    it(`highlights ${language}`, () => {
      const lines = code.split("\n");
      const tokens = highlightLines(lines, language);
      expect(tokens).toHaveLength(lines.length);
      const allTokens = tokens.flat();
      expect(allTokens.length, `expected tokens for ${language}`).toBeGreaterThan(0);
    });
  }

  it("marks a Rust keyword", () => {
    const tokens = highlightLines(["fn main() {}"], "rust");
    expect(classesOf(tokens, 0).some((c) => c.includes("keyword"))).toBe(true);
  });

  it("gives one entry per input line", () => {
    expect(highlightLines(["let a = 1;", "let b = 2;"], "rust")).toHaveLength(2);
  });

  it("keeps offsets relative to their own line", () => {
    const tokens = highlightLines(["let a = 1;", "fn main() {}"], "rust");
    const keyword = tokens[1]?.find((t) => t.cls.includes("keyword"));
    expect(keyword?.start).toBe(0);
    expect(keyword?.end).toBe(2);
  });

  it("carries a block comment across the line it started on", () => {
    const tokens = highlightLines(["/* start", "still comment */", "let a = 1;"], "rust");

    expect(classesOf(tokens, 1).some((c) => c.includes("comment"))).toBe(true);
  });

  it("does not highlight beyond the end of a line", () => {
    const lines = ["let a = 1;", "let bb = 2;"];
    const tokens = highlightLines(lines, "rust");
    for (const [index, line] of lines.entries()) {
      for (const token of tokens[index] ?? []) {
        expect(token.end).toBeLessThanOrEqual(line.length);
        expect(token.start).toBeGreaterThanOrEqual(0);
      }
    }
  });

  it("gives up on a file too large to be worth parsing", () => {
    const many = Array.from({ length: MAX_HIGHLIGHT_LINES + 1 }, () => "let a = 1;");
    expect(highlightLines(many, "rust").every((line) => line.length === 0)).toBe(true);
  });

  it("handles an empty input", () => {
    expect(highlightLines([], "rust")).toEqual([]);
  });

  it("survives text that does not parse", () => {
    expect(() => highlightLines(["!!! not rust @@@"], "rust")).not.toThrow();
  });

  // DF-053: without the JSX dialect `</div>` opened a regular expression and the colours
  // after the tag were wrong.
  it("reads JSX tags in .tsx and .jsx as tags, not as a regular expression", () => {
    const line = 'return <div className="a">x</div>;';
    for (const language of ["tsx", "jsx"]) {
      const tokens = highlightLines([line], language)[0] ?? [];
      const at = (text: string) => tokens.find((t) => t.start === line.indexOf(text))?.cls;
      expect(at("div"), language).toBe("tok-typeName");
      expect(tokens.some((t) => t.cls.includes("string2")), language).toBe(false);
    }
  });

  describe("languageOf", () => {
    it("maps standard file extensions", () => {
      expect(languageOf("src/main.rs")).toBe("rust");
      expect(languageOf("app.ts")).toBe("typescript");
      expect(languageOf("app.tsx")).toBe("tsx");
      expect(languageOf("script.js")).toBe("javascript");
      expect(languageOf("script.mjs")).toBe("javascript");
      expect(languageOf("script.cjs")).toBe("javascript");
      expect(languageOf("component.jsx")).toBe("jsx");
      expect(languageOf("script.py")).toBe("python");
      expect(languageOf("script.pyi")).toBe("python");
      expect(languageOf("main.c")).toBe("c");
      expect(languageOf("main.h")).toBe("c");
      expect(languageOf("main.cpp")).toBe("c++");
      expect(languageOf("main.java")).toBe("java");
      expect(languageOf("main.go")).toBe("go");
      expect(languageOf("Program.cs")).toBe("c#");
      expect(languageOf("script.rb")).toBe("ruby");
      expect(languageOf("index.php")).toBe("php");
      expect(languageOf("Main.swift")).toBe("swift");
      expect(languageOf("Main.kt")).toBe("kotlin");
      expect(languageOf("script.sh")).toBe("shell");
      expect(languageOf("query.sql")).toBe("sql");
      expect(languageOf("data.json")).toBe("json");
      expect(languageOf("config.yaml")).toBe("yaml");
      expect(languageOf("config.yml")).toBe("yaml");
      expect(languageOf("Cargo.toml")).toBe("toml");
      expect(languageOf("doc.xml")).toBe("xml");
      expect(languageOf("image.svg")).toBe("xml");
      expect(languageOf("index.html")).toBe("html");
      expect(languageOf("style.css")).toBe("css");
      expect(languageOf("style.scss")).toBe("scss");
      expect(languageOf("style.sass")).toBe("sass");
      expect(languageOf("README.md")).toBe("markdown");
      expect(languageOf("App.svelte")).toBe("svelte");
      expect(languageOf("App.vue")).toBe("vue");
      expect(languageOf("CMakeLists.txt")).toBe("cmake");
      expect(languageOf("project.cmake")).toBe("cmake");
      expect(languageOf("Dockerfile")).toBe("dockerfile");
      expect(languageOf("app.dockerfile")).toBe("dockerfile");
      expect(languageOf("script.lua")).toBe("lua");
      expect(languageOf("script.pl")).toBe("perl");
      expect(languageOf("Module.pm")).toBe("perl");
      expect(languageOf("calc.r")).toBe("r");
      expect(languageOf("App.scala")).toBe("scala");
      expect(languageOf("main.dart")).toBe("dart");
      expect(languageOf("Main.hs")).toBe("haskell");
      expect(languageOf("build.gradle")).toBe("groovy");
      expect(languageOf("deploy.ps1")).toBe("powershell");
    });

    it("returns null for unknown extensions or files without extension", () => {
      expect(languageOf("notes.xyz")).toBeNull();
      expect(languageOf("LICENSE")).toBeNull();
      expect(languageOf(".gitignore")).toBeNull();
    });
  });
});

describe("mergePieces", () => {
  it("returns the whole line when there is nothing to mark", () => {
    expect(mergePieces("plain", [], [])).toEqual([{ text: "plain", cls: "", changed: false, hit: false, start: 0 }]);
  });

  it("applies a syntax class to its own range only", () => {
    const pieces = mergePieces("fn x", [{ start: 0, end: 2, cls: "tok-keyword" }], []);
    expect(pieces.map((p) => p.cls)).toEqual(["tok-keyword", ""]);
  });

  it("marks a changed word without losing its syntax class", () => {
    const pieces = mergePieces("let a", [{ start: 0, end: 3, cls: "tok-keyword" }], [[0, 3]]);
    expect(pieces[0]).toEqual({ text: "let", cls: "tok-keyword", changed: true, hit: false, start: 0 });
  });

  it("cuts at every boundary either overlay introduces", () => {
    const pieces = mergePieces("abcdef", [{ start: 0, end: 4, cls: "t" }], [[2, 6]]);
    expect(pieces.map((p) => p.text)).toEqual(["ab", "cd", "ef"]);
    expect(pieces.map((p) => p.changed)).toEqual([false, true, true]);
  });

  it("puts the pieces back together into the original text", () => {
    const text = "const answer = 42;";
    const pieces = mergePieces(text, [{ start: 0, end: 5, cls: "tok-keyword" }], [[15, 17]]);
    expect(pieces.map((p) => p.text).join("")).toBe(text);
  });

  it("clamps a range that runs past the end", () => {
    expect(mergePieces("ab", [], [[1, 99]]).map((p) => p.text).join("")).toBe("ab");
  });

  it("handles an empty line", () => {
    expect(mergePieces("", [], [])).toEqual([]);
  });

  it("marks a search hit and nothing around it", () => {
    const pieces = mergePieces("find me here", [], [], [[5, 7]]);

    expect(pieces.map((p) => p.text)).toEqual(["find ", "me", " here"]);
    expect(pieces.map((p) => p.hit)).toEqual([false, true, false]);
  });

  it("keeps a hit that lands inside a changed word marked as both", () => {
    const pieces = mergePieces("alpha", [], [[0, 5]], [[0, 5]]);

    expect(pieces[0]).toEqual({ text: "alpha", cls: "", changed: true, hit: true, start: 0 });
  });

  it("cuts at the hit boundary as well as the other two", () => {
    const pieces = mergePieces("abcdef", [{ start: 0, end: 2, cls: "t" }], [[2, 4]], [[3, 6]]);

    expect(pieces.map((p) => p.text).join("")).toBe("abcdef");
    expect(pieces.map((p) => p.hit)).toEqual([false, false, true, true]);
  });
});

describe("mergePieces on a minified line", () => {
  // One line of minified JavaScript: tens of thousands of alternating tokens. Every piece
  // searched the whole token list, and the window froze for seconds (rule of 50 ms).
  const line = "a=1;".repeat(75_000);
  const tokens = Array.from({ length: 150_000 }, (_, i) => ({
    start: i * 2,
    end: i * 2 + 1,
    cls: i % 2 ? "tok-number" : "tok-variableName",
  }));

  it("cuts a 300 KB line in one pass", () => {
    const started = performance.now();
    mergePieces(line, tokens, [[10, 20]], [[100, 104]]);
    expect(performance.now() - started).toBeLessThan(1000);
  });

  it("keeps the changed words and the hits of a line too long to colour", () => {
    const pieces = mergePieces(line, tokens, [[10, 20]], [[100, 104]]);

    expect(pieces.every((piece) => piece.cls === "")).toBe(true);
    expect(pieces.filter((piece) => piece.changed).map((piece) => piece.text).join("")).toBe(line.slice(10, 20));
    expect(pieces.filter((piece) => piece.hit).map((piece) => piece.text).join("")).toBe(line.slice(100, 104));
    expect(pieces.map((piece) => piece.text).join("")).toBe(line);
  });

  it("colours a short line exactly as before", () => {
    const text = "let x = 1;";
    const short = [
      { start: 0, end: 3, cls: "tok-keyword" },
      { start: 4, end: 5, cls: "tok-variableName" },
      { start: 8, end: 9, cls: "tok-number" },
    ];
    const pieces = mergePieces(text, short, [[4, 9]], [[8, 10]]);

    expect(pieces).toEqual([
      { text: "let", cls: "tok-keyword", changed: false, hit: false, start: 0 },
      { text: " ", cls: "", changed: false, hit: false, start: 3 },
      { text: "x", cls: "tok-variableName", changed: true, hit: false, start: 4 },
      { text: " = ", cls: "", changed: true, hit: false, start: 5 },
      { text: "1", cls: "tok-number", changed: true, hit: true, start: 8 },
      { text: ";", cls: "", changed: false, hit: true, start: 9 },
    ]);
  });
});
