import { describe, expect, it } from "vitest";
import type { Branch, CiStatus } from "$lib/ipc";
import {
  branchTargets,
  checkLine,
  ciMenu,
  ciTooltip,
  durationText,
  graphTargets,
  menuUrl,
  mergeTargets,
  nextPoll,
  remoteOf,
  shown,
} from "./ci-status";

function branch(name: string, kind: "local" | "remote", oid: string, upstream: string | null = null, ahead = 0): Branch {
  return { name, fullName: name, kind, oid, isHead: false, upstream, pushRemote: null, pushTarget: null, ahead, behind: 0 };
}

const status: CiStatus = {
  sha: "abc",
  state: "failure",
  checks: [
    { name: "test", state: "failure", description: null, url: "https://ci/test", durationSecs: 125 },
    { name: "lint", state: "success", description: null, url: null, durationSecs: null },
  ],
  url: "https://github.com/o/r/commit/abc",
  note: null,
};

describe("which commits are asked for", () => {
  it("only the rows on screen, never one no remote has", () => {
    const targets = graphTargets(["a", "b", "a", "c"], (oid) => oid === "b");
    expect(targets).toEqual([
      { sha: "a", remote: null },
      { sha: "c", remote: null },
    ]);
  });

  it("stops at the limit", () => {
    expect(graphTargets(["a", "b", "c"], () => false, 2).map((t) => t.sha)).toEqual(["a", "b"]);
  });

  it("a branch asks its upstream's remote; a local one ahead of it or without one is skipped", () => {
    const targets = branchTargets(
      [
        branch("main", "local", "m", "origin/main"),
        branch("wip", "local", "w", "origin/wip", 2),
        branch("solo", "local", "s"),
        branch("up/stream/x", "remote", "u"),
        branch("origin/main", "remote", "m"),
        branch("ghost/x", "remote", "g"),
      ],
      ["origin", "up", "up/stream"],
    );
    expect(targets).toEqual([
      { sha: "m", remote: "origin" },
      { sha: "u", remote: "up/stream" },
    ]);
  });

  it("the longest remote name wins", () => {
    expect(remoteOf("a/b/c", ["a", "a/b"])).toBe("a/b");
    expect(remoteOf("main", ["origin"])).toBeNull();
  });

  it("graph and branches merge without asking a commit twice", () => {
    expect(mergeTargets([{ sha: "a", remote: null }], [{ sha: "a", remote: "up" }, { sha: "b", remote: "up" }])).toEqual([
      { sha: "a", remote: null },
      { sha: "b", remote: "up" },
    ]);
  });
});

describe("polling", () => {
  it("never faster than three seconds nor slower than ten minutes", () => {
    expect(nextPoll(null)).toBeNull();
    expect(nextPoll(500)).toBe(3_000);
    expect(nextPoll(15_000)).toBe(15_000);
    expect(nextPoll(4_000_000)).toBe(600_000);
  });
});

describe("what the icon shows", () => {
  it("no icon for a commit without checks", () => {
    expect(shown(undefined)).toBe(false);
    expect(shown({ ...status, state: "none", checks: [] })).toBe(false);
    expect(shown(status)).toBe(true);
  });

  it("durations read like a clock", () => {
    expect(durationText(45)).toBe("45 s");
    expect(durationText(120)).toBe("2 min");
    expect(durationText(125)).toBe("2 min 5 s");
    expect(durationText(3_780)).toBe("1 h 3 min");
  });

  it("the tooltip lists every check and the quiet reason", () => {
    expect(checkLine(status.checks[0]!)).toBe("✗ test — failed, 2 min 5 s");
    const text = ciTooltip({ ...status, note: "GitHub rate limit reached" });
    expect(text.split("\n")).toEqual([
      "CI failed: 2 checks",
      "✗ test — failed, 2 min 5 s",
      "✓ lint — passed",
      "GitHub rate limit reached",
      "Click for links",
    ]);
  });

  it("the menu opens a check, or the whole run; a check without a link is off", () => {
    const menu = ciMenu(status);
    const rows = menu.items.filter((row) => !row.separator);
    expect(rows.map((row) => [row.label, row.enabled])).toEqual([
      ["✗ test — failed, 2 min 5 s", true],
      ["✓ lint — passed", false],
      ["Open All Checks in Browser", true],
    ]);
    expect(menuUrl(menu, rows[0]!.id)).toBe("https://ci/test");
    expect(menuUrl(menu, rows[2]!.id)).toBe("https://github.com/o/r/commit/abc");
    expect(menuUrl(menu, "copy-sha")).toBeUndefined();
    expect(menuUrl(null, "ci-open:0")).toBeNull();
  });
});
