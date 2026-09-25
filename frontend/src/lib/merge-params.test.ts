import { describe, expect, it } from "vitest";
import { CogitError } from "./ipc";
import { failureText, mergeUrl, parseMerge } from "./merge-params";

describe("mergeUrl", () => {
  it("points at the merge entry point", () => {
    expect(mergeUrl(1, "a.txt").startsWith("merge.html?")).toBe(true);
  });

  it("survives a path with characters a query string would eat", () => {
    const url = mergeUrl(1, "src/a b&c.txt");
    expect(parseMerge(url.slice(url.indexOf("?")))?.path).toBe("src/a b&c.txt");
  });
});

describe("parseMerge", () => {
  it("reads back what was written", () => {
    const url = mergeUrl(7, "deep/nested/file.rs");
    expect(parseMerge(url.slice(url.indexOf("?")))).toEqual({ repo: 7, path: "deep/nested/file.rs" });
  });

  it("is null without a path", () => {
    expect(parseMerge("?repo=1")).toBeNull();
  });

  it("is null without a repository", () => {
    expect(parseMerge("?path=a.txt")).toBeNull();
  });

  it("is null for a repository id that is not a number", () => {
    expect(parseMerge("?repo=abc&path=a.txt")).toBeNull();
  });

  it("is null for a repository id no repository could have", () => {
    expect(parseMerge("?repo=0&path=a.txt")).toBeNull();
  });
});

describe("failureText", () => {
  // String(err) gave only "git add -- a.txt failed with exit code 128": git's reason was lost.
  it("keeps git's own output of a command that failed", () => {
    const err = new CogitError({
      kind: "command",
      data: {
        id: 3,
        repo: "C:/work/app",
        command: "git add -- a.txt",
        exitCode: 128,
        stdout: "",
        stderr: "fatal: Unable to create 'C:/work/app/.git/index.lock': File exists.",
        operation: "Stage",
        summary: "fatal: Unable to create 'C:/work/app/.git/index.lock': File exists.",
      },
    });
    const text = failureText(err);
    expect(text).toContain("git add -- a.txt");
    expect(text).toContain("128");
    expect(text).toContain("index.lock': File exists.");
  });

  it("says what anything else said", () => {
    expect(failureText(new CogitError({ kind: "io", data: "os error 32" }))).toContain("os error 32");
  });
});
