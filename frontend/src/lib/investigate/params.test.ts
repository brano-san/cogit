import { describe, expect, it } from "vitest";
import { investigateTitle, investigateUrl, parseInvestigate } from "./params";

const parse = (url: string) => parseInvestigate(new URL(url, "http://x/").search);

describe("investigateUrl and parseInvestigate", () => {
  it("round-trips a commit, a line and the repository name", () => {
    const request = {
      repo: 3,
      repoName: "cogit",
      start: { path: "src/a.rs", rev: "abc123", line: 42 },
    };
    expect(parse(investigateUrl(request))).toEqual(request);
  });

  it("reads a missing revision as the working tree and a missing line as none", () => {
    const url = investigateUrl({
      repo: 1,
      repoName: "r",
      start: { path: "a.txt", rev: null, line: null },
    });
    expect(parse(url)?.start).toEqual({ path: "a.txt", rev: null, line: null });
  });

  it("survives spaces, non-ASCII, question marks and ampersands", () => {
    const path = "dir/файл a?b&c.txt";
    const url = investigateUrl({ repo: 1, repoName: "x&y", start: { path, rev: null, line: 1 } });
    expect(parse(url)?.start.path).toBe(path);
    expect(parse(url)?.repoName).toBe("x&y");
  });

  it("refuses a URL without a repository or a path", () => {
    expect(parseInvestigate("?path=a.txt")).toBeNull();
    expect(parseInvestigate("?repo=1")).toBeNull();
    expect(parseInvestigate("?repo=0&path=a")).toBeNull();
  });

  it("ignores a line that is not a positive whole number", () => {
    expect(parseInvestigate("?repo=1&path=a&line=-4")?.start.line).toBeNull();
    expect(parseInvestigate("?repo=1&path=a&line=x")?.start.line).toBeNull();
  });
});

describe("investigateTitle", () => {
  it("names the file, the repository and the tool", () => {
    expect(investigateTitle("src/lib/main.rs", "cogit")).toBe("main.rs [cogit] - Investigate");
  });

  it("leaves the brackets out without a repository name", () => {
    expect(investigateTitle("a.txt", "")).toBe("a.txt - Investigate");
  });
});
