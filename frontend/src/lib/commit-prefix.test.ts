import { describe, expect, it } from "vitest";
import {
  amendedMessage,
  branchPrefix,
  effectiveRule,
  headBranch,
  messageSeed,
  patternProblem,
  repoRule,
} from "./commit-prefix";

const JIRA = { pattern: "^(?:feature|bugfix)/([A-Z]+-\\d+)", template: "$1: " };

describe("branchPrefix", () => {
  it("turns a ticket branch into its prefix", () => {
    expect(branchPrefix(JIRA, "feature/ABC-123-login")).toBe("ABC-123: ");
    expect(branchPrefix(JIRA, "bugfix/XY-7")).toBe("XY-7: ");
  });

  it("gives nothing for a branch that does not match, no branch or no rule", () => {
    expect(branchPrefix(JIRA, "main")).toBeNull();
    expect(branchPrefix(JIRA, null)).toBeNull();
    expect(branchPrefix({ pattern: "  ", template: "$1" }, "feature/ABC-1")).toBeNull();
  });

  it("expands every group up to $9, the whole match and a literal dollar", () => {
    const rule = { pattern: "^(\\w+)/(\\w+)-(\\d+)", template: "[$2 #$3] $$ $& $9" };
    expect(branchPrefix(rule, "team/core-42-x")).toBe("[core #42] $ team/core-42 ");
  });

  it("gives nothing for an invalid pattern or a template that comes out empty", () => {
    expect(branchPrefix({ pattern: "([A-Z", template: "$1" }, "feature/ABC")).toBeNull();
    expect(branchPrefix({ pattern: "^feature/(x)?", template: "$1" }, "feature/abc")).toBeNull();
  });
});

describe("patternProblem", () => {
  it("accepts a valid or an empty pattern", () => {
    expect(patternProblem(JIRA.pattern)).toBeNull();
    expect(patternProblem("")).toBeNull();
  });

  it("says why a pattern is not a regular expression", () => {
    expect(patternProblem("([A-Z")).toMatch(/^Not a valid regular expression: /);
  });
});

describe("the rule in effect", () => {
  const settings = [
    { key: "cogit.commitPrefixPattern", local: "^(\\d+)-", inherited: null },
    { key: "cogit.commitPrefixTemplate", local: null, inherited: null },
  ];

  it("takes the repository's pattern and Preferences' template when only the pattern is set", () => {
    expect(effectiveRule(JIRA, repoRule(settings))).toEqual({ pattern: "^(\\d+)-", template: "$1: " });
  });

  it("follows Preferences when the repository sets nothing", () => {
    expect(effectiveRule(JIRA, repoRule([]))).toEqual(JIRA);
  });

  it("turns the prefix off for a repository that sets an empty pattern", () => {
    const off = effectiveRule(JIRA, repoRule([{ key: "cogit.commitPrefixPattern", local: "", inherited: "x" }]));
    expect(branchPrefix(off, "feature/ABC-1")).toBeNull();
  });
});

describe("headBranch", () => {
  it("names a branch, born or not, and nothing when detached", () => {
    expect(headBranch({ kind: "branch", name: "feature/A-1", oid: "1" })).toBe("feature/A-1");
    expect(headBranch({ kind: "unborn", name: "refs/heads/feature/A-1" })).toBe("feature/A-1");
    expect(headBranch({ kind: "detached", oid: "1" })).toBeNull();
    expect(headBranch(null)).toBeNull();
  });
});

describe("what the field holds", () => {
  it("never mixes the prefix into what git prepared or commit.template", () => {
    expect(messageSeed("Merge branch 'x'\n", "ABC-1: ")).toBe("Merge branch 'x'\n");
    expect(messageSeed(null, "ABC-1: ")).toBe("ABC-1: ");
    expect(messageSeed(null, null)).toBeNull();
  });

  it("prefixes an amended message once", () => {
    expect(amendedMessage("fix login", "ABC-1: ")).toBe("ABC-1: fix login");
    expect(amendedMessage("ABC-1: fix login", "ABC-1: ")).toBe("ABC-1: fix login");
    expect(amendedMessage("ABC-1:fix", "ABC-1: ")).toBe("ABC-1:fix");
    expect(amendedMessage("fix", null)).toBe("fix");
  });
});
