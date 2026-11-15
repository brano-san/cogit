import { describe, expect, it } from "vitest";
import { findIssues, issueLinks, issueRule, issueUrl, type IssueTrackerConfig } from "./issue-links";

const tracker = (over: Partial<IssueTrackerConfig>): IssueTrackerConfig => ({
  origin: "config",
  url: "https://tracker.example/issue/%BUGID%",
  logRegex: null,
  message: null,
  ...over,
});

const ids = (text: string, config: Partial<IssueTrackerConfig>) =>
  findIssues(text, issueRule([tracker(config)])!).map((each) => each.id);

describe("issueRule", () => {
  it("is null with no configuration at all", () => {
    expect(issueRule([])).toBeNull();
    expect(issueLinks("Fix #12", null)).toEqual([{ text: "Fix #12", issue: null, href: null }]);
  });

  it("needs a web URL: a rule from a cloned repository must not open a file or a program", () => {
    expect(issueRule([tracker({ url: null, logRegex: "#(\\d+)" })])).toBeNull();
    expect(issueRule([tracker({ url: "file:///C:/x.exe?%BUGID%", logRegex: "#(\\d+)" })])).toBeNull();
  });

  it("skips a place with a broken regex for the next one", () => {
    const rule = issueRule([
      tracker({ origin: "cogit", logRegex: "#(\\d+" }),
      tracker({ origin: ".tgitconfig", url: "https://t/%BUGID%", logRegex: "#(\\d+)" }),
    ]);
    expect(rule?.url).toBe("https://t/%BUGID%");
  });

  it("takes the first usable place", () => {
    const rule = issueRule([
      tracker({ url: "https://first/%BUGID%", logRegex: "[A-Z]+-\\d+" }),
      tracker({ url: "https://second/%BUGID%", logRegex: "#(\\d+)" }),
    ]);
    expect(rule?.url).toBe("https://first/%BUGID%");
  });
});

describe("one regex", () => {
  it("links the groups", () => {
    expect(ids("Fixes #12 and #345.", { logRegex: "#(\\d+)" })).toEqual(["12", "345"]);
  });

  it("links the whole match when the regex has no group", () => {
    expect(ids("PROJ-7: login, see also PROJ-8", { logRegex: "[A-Z]+-\\d+" })).toEqual(["PROJ-7", "PROJ-8"]);
  });

  it("finds nothing in a message without ids", () => {
    expect(ids("Refactor the parser", { logRegex: "#(\\d+)" })).toEqual([]);
  });
});

describe("TortoiseGit's two-regex form", () => {
  const logRegex = "[Ii]ssues?:?(\\s*(,|and)?\\s*#\\d+)+\n(\\d+)";

  it("links every id inside the mention, and nothing outside it", () => {
    expect(ids("Issues #1, #22 and #333 fixed; build #9 green", { logRegex })).toEqual(["1", "22", "333"]);
  });

  it("puts each link exactly over its id", () => {
    const parts = issueLinks("issue #7 done", issueRule([tracker({ logRegex })]));
    expect(parts).toEqual([
      { text: "issue #", issue: null, href: null },
      { text: "7", issue: "7", href: "https://tracker.example/issue/7" },
      { text: " done", issue: null, href: null },
    ]);
  });

  it("accepts Windows line endings between the two", () => {
    expect(ids("Issue #5", { logRegex: logRegex.replace("\n", "\r\n") })).toEqual(["5"]);
  });
});

describe("bugtraq.message without a regex", () => {
  it("finds the ids where the message puts them", () => {
    expect(ids("Fix the crash\n\nIssue: 12, 13", { message: "Issue: %BUGID%" })).toEqual(["12", "13"]);
    expect(ids("Fix (#ABC-4)", { message: "(#%BUGID%)" })).toEqual(["ABC-4"]);
  });

  it("ignores a message without %BUGID%", () => {
    expect(issueRule([tracker({ message: "Issue:" })])).toBeNull();
  });
});

describe("issueUrl", () => {
  it("puts the id into every %BUGID%, encoded", () => {
    expect(issueUrl("https://t/%BUGID%?ref=%BUGID%", "A B")).toBe("https://t/A%20B?ref=A%20B");
  });

  it("keeps the text whole around several links", () => {
    const parts = issueLinks("#1 and #2", issueRule([tracker({ logRegex: "#(\\d+)" })]));
    expect(parts.map((part) => part.text).join("")).toBe("#1 and #2");
    expect(parts.filter((part) => part.href).map((part) => part.href)).toEqual([
      "https://tracker.example/issue/1",
      "https://tracker.example/issue/2",
    ]);
  });
});
