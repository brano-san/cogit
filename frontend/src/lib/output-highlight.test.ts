import { describe, expect, it } from "vitest";
import { classifyLine, findMatches, highlightStream, logLines } from "./output-highlight";

describe("classifyLine", () => {
  it("calls an error an error", () => {
    expect(classifyLine("error: pathspec 'nope' did not match")).toBe("error");
  });

  it("calls fatal an error", () => {
    expect(classifyLine("fatal: not a git repository")).toBe("error");
  });

  it("calls a conflict an error", () => {
    expect(classifyLine("CONFLICT (content): Merge conflict in a.txt")).toBe("error");
  });

  it("calls a warning a warning", () => {
    expect(classifyLine("warning: LF will be replaced by CRLF")).toBe("warning");
  });

  it("calls a hint a warning", () => {
    // Same colour: a hint is git telling the user something went sideways.
    expect(classifyLine("hint: Updates were rejected")).toBe("warning");
  });

  it("sees through the remote prefix", () => {
    expect(classifyLine("remote: error: hook declined")).toBe("error");
    expect(classifyLine("remote: warning: deprecated endpoint")).toBe("warning");
  });

  it("ignores leading spaces", () => {
    expect(classifyLine("   warning: something")).toBe("warning");
  });

  it("ignores the case git writes it in", () => {
    expect(classifyLine("ERROR: broken")).toBe("error");
  });

  it("leaves an ordinary line alone", () => {
    expect(classifyLine("Switched to branch 'main'")).toBe("plain");
  });

  it("does not colour a line that merely mentions the word", () => {
    expect(classifyLine("Fixed the error in the parser")).toBe("plain");
  });

  it("treats a blank line as plain", () => {
    expect(classifyLine("")).toBe("plain");
  });
});

describe("highlightStream", () => {
  it("returns one entry per line, in order", () => {
    const rows = highlightStream("one\ntwo\nthree");
    expect(rows.map((row) => row.text)).toEqual(["one", "two", "three"]);
  });

  it("marks each line with its own kind", () => {
    const rows = highlightStream("ok\nwarning: careful\nfatal: stop");
    expect(rows.map((row) => row.kind)).toEqual(["plain", "warning", "error"]);
  });

  it("keeps a trailing newline from inventing an empty row", () => {
    expect(highlightStream("one\n")).toHaveLength(1);
  });

  it("keeps blank lines inside the text, which carry the shape of the output", () => {
    expect(highlightStream("one\n\ntwo")).toHaveLength(3);
  });

  it("survives carriage returns from a Windows git", () => {
    const rows = highlightStream("warning: careful\r\nok");
    expect(rows[0]?.kind).toBe("warning");
    expect(rows[0]?.text).toBe("warning: careful");
  });
});

describe("classifyLine, on what a hook prints", () => {
  it("marks a panic, which is the line the reader is looking for", () => {
    expect(classifyLine("thread 'main' panicked at crates/a/src/b.rs:12:5:")).toBe("error");
  });

  it("marks a failed test", () => {
    expect(classifyLine("FAILED [   0.31s] git_engine::runner captures_stdout")).toBe("error");
  });

  it("leaves a passing test alone, or the whole log turns red", () => {
    expect(classifyLine("PASS [   0.31s] git_engine::runner captures_stdout")).toBe("plain");
  });

  it("marks the line that says output is missing", () => {
    expect(classifyLine("… 23000 lines omitted, see log …")).toBe("omitted");
  });
});

describe("logLines", () => {
  it("puts stderr first, because that is where the reason is", () => {
    const rows = logLines("on stdout", "on stderr");
    expect(rows.map((row) => row.text)).toEqual([
      "stderr",
      "on stderr",
      "stdout",
      "on stdout",
    ]);
  });

  it("labels the sections so the two streams cannot be confused", () => {
    expect(logLines("out", "err").map((row) => row.kind)).toEqual([
      "label",
      "plain",
      "label",
      "plain",
    ]);
  });

  it("does not label a stream that is empty", () => {
    expect(logLines("", "fatal: no").map((row) => row.text)).toEqual(["fatal: no"]);
  });

  it("is empty when the command said nothing at all", () => {
    expect(logLines("", "")).toEqual([]);
  });

  it("keeps every line of a long log", () => {
    const log = Array.from({ length: 20_000 }, (_, i) => `line ${i}`).join("\n");
    expect(logLines(log, "")).toHaveLength(20_000);
  });
});

describe("findMatches", () => {
  const rows = logLines("", "fatal: one\nplain two\nFATAL: three");

  it("gives the index of every line that contains the text", () => {
    expect(findMatches(rows, "fatal")).toEqual([0, 2]);
  });

  it("ignores case, the way a reader skimming a log does", () => {
    expect(findMatches(rows, "FATAL")).toEqual([0, 2]);
  });

  it("finds nothing for an empty needle, rather than everything", () => {
    expect(findMatches(rows, "")).toEqual([]);
  });

  it("treats the needle as text, not as a pattern", () => {
    const dotted = logLines("", "a.b\naxb");
    expect(findMatches(dotted, "a.b")).toEqual([0]);
  });
});
