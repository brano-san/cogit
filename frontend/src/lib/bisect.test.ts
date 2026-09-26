import { describe, expect, it } from "vitest";
import {
  BISECT_MENU_PREFIX,
  bisectBanner,
  bisectCommands,
  bisectCommitMenu,
  bisectMenuChoice,
  bisectPhase,
  newlyFound,
  resetQuestion,
  startBlocked,
  startFields,
  startProblem,
} from "./bisect";
import type { BisectState } from "./ipc/bisect";
import type { ContextItem, RepoState } from "./ipc";
import { graphCommitMenu } from "./ref-menus";

const A = "a".repeat(40);
const B = "b".repeat(40);
const C = "c".repeat(40);
const D = "d".repeat(40);

function state(over: Partial<BisectState> = {}): BisectState {
  return {
    start: "main",
    bad: null,
    good: [],
    skipped: [],
    current: C,
    firstBad: null,
    candidates: [],
    terms: { bad: "bad", good: "good" },
    ...over,
  };
}

const bisecting = (over: Partial<BisectState> = {}): RepoState => ({ kind: "bisecting", bisect: state(over) });

function rows(item: ContextItem): Record<string, string> {
  return Object.fromEntries((item.children ?? []).filter((row) => !row.separator).map((row) => [row.id, row.label]));
}

describe("bisectPhase", () => {
  it("waits for marks until both a bad and a good commit are known", () => {
    expect(bisectPhase(state())).toBe("waiting");
    expect(bisectPhase(state({ bad: A }))).toBe("needGood");
    expect(bisectPhase(state({ good: [B] }))).toBe("needBad");
    expect(bisectPhase(state({ bad: A, good: [B] }))).toBe("testing");
  });

  it("ends when git names the first bad commit, or only skipped ones are left", () => {
    expect(bisectPhase(state({ bad: A, good: [B], firstBad: A }))).toBe("found");
    expect(bisectPhase(state({ bad: A, good: [B], candidates: [C, A] }))).toBe("stuck");
  });
});

describe("startBlocked", () => {
  it("lets a bisect start on a branch or a detached HEAD", () => {
    expect(startBlocked({ kind: "clean" })).toBeNull();
    expect(startBlocked({ kind: "detachedHead", oid: A })).toBeNull();
  });

  it("says why it cannot start anywhere else", () => {
    expect(startBlocked(bisecting())).toBe("a bisect is in progress");
    expect(startBlocked({ kind: "merging" })).toBe("another operation is in progress");
    expect(startBlocked({ kind: "bare" })).toBe("a bare repository");
    expect(startBlocked({ kind: "empty" })).toBe("no commits yet");
    expect(startBlocked(null)).toBe("no repository");
  });
});

describe("the Bisect submenu of a commit", () => {
  it("offers only Start while no bisect runs, and says why the rest is off", () => {
    const menu = bisectCommitMenu({ kind: "clean" }, A);
    expect(menu.label).toBe("Bisect");
    const labels = rows(menu);
    expect(labels[`${BISECT_MENU_PREFIX}start:${A}`]).toBe("Start…");
    expect(labels[`${BISECT_MENU_PREFIX}good:${A}`]).toBe("Mark as Good (no bisect in progress)");
    expect(labels[`${BISECT_MENU_PREFIX}reset:${A}`]).toBe("Reset (no bisect in progress)");
  });

  it("marks any commit while one runs, and not twice the same way", () => {
    const labels = rows(bisectCommitMenu(bisecting({ bad: A, good: [B], skipped: [D] }), B));
    expect(labels[`${BISECT_MENU_PREFIX}start:${B}`]).toBe("Start… (a bisect is in progress)");
    expect(labels[`${BISECT_MENU_PREFIX}good:${B}`]).toBe("Mark as Good (already good)");
    expect(labels[`${BISECT_MENU_PREFIX}bad:${B}`]).toBe("Mark as Bad");
    expect(labels[`${BISECT_MENU_PREFIX}skip:${B}`]).toBe("Skip");
    expect(labels[`${BISECT_MENU_PREFIX}reset:${B}`]).toBe("Reset");
    expect(rows(bisectCommitMenu(bisecting({ bad: A }), A))[`${BISECT_MENU_PREFIX}bad:${A}`]).toBe(
      "Mark as Bad (already bad)",
    );
    expect(rows(bisectCommitMenu(bisecting({ skipped: [D] }), D))[`${BISECT_MENU_PREFIX}skip:${D}`]).toBe(
      "Skip (already skipped)",
    );
  });

  it("brings the chosen row back with the commit it was opened on", () => {
    expect(bisectMenuChoice(`${BISECT_MENU_PREFIX}good:${A}`)).toEqual({ action: "good", oid: A });
    expect(bisectMenuChoice(`${BISECT_MENU_PREFIX}start:${B}`)).toEqual({ action: "start", oid: B });
    expect(bisectMenuChoice("ref:checkout")).toBeNull();
    expect(bisectMenuChoice(`${BISECT_MENU_PREFIX}merge:${A}`)).toBeNull();
    expect(bisectMenuChoice(`${BISECT_MENU_PREFIX}good`)).toBeNull();
  });
});

describe("Branch ▸ Bisect and the palette", () => {
  const ids = (commands: ReturnType<typeof bisectCommands>) =>
    Object.fromEntries(commands.map((command) => [command.id, command.unavailable]));

  it("has the ids of the native menu", () => {
    expect(bisectCommands({ kind: "clean" }, () => {}).map((command) => command.id)).toEqual([
      "bisect-start",
      "bisect-good",
      "bisect-bad",
      "bisect-skip",
      "bisect-reset",
    ]);
  });

  it("starts outside a bisect and marks HEAD inside one", () => {
    expect(ids(bisectCommands({ kind: "clean" }, () => {}))).toEqual({
      "bisect-start": undefined,
      "bisect-good": "No bisect in progress",
      "bisect-bad": "No bisect in progress",
      "bisect-skip": "No bisect in progress",
      "bisect-reset": "No bisect in progress",
    });
    const inside = ids(bisectCommands(bisecting({ bad: C }), () => {}));
    expect(inside["bisect-start"]).toBe("A bisect is in progress");
    expect(inside["bisect-good"]).toBeUndefined();
    expect(inside["bisect-bad"]).toBe("Already bad");
    expect(inside["bisect-reset"]).toBeUndefined();
  });

  it("hands the action to the component that runs it", () => {
    const ran: string[] = [];
    for (const command of bisectCommands(bisecting(), (action) => ran.push(action))) command.run();
    expect(ran).toEqual(["start", "good", "bad", "skip", "reset"]);
  });
});

describe("the Start dialog", () => {
  it("takes the commit it was opened on as the good one, HEAD as the bad", () => {
    expect(startFields(B, A)).toEqual({ bad: "HEAD", good: B });
    expect(startFields(A, A)).toEqual({ bad: "HEAD", good: "" });
    expect(startFields(null, A)).toEqual({ bad: "HEAD", good: "" });
  });

  it("needs a bad commit and lets the good one wait", () => {
    expect(startProblem("", "")).toContain("bad commit");
    expect(startProblem("HEAD", "")).toBeNull();
    expect(startProblem("HEAD", "v1.0")).toBeNull();
    expect(startProblem("v1.0", " v1.0 ")).toContain("differ");
  });
});

describe("the bisect banner", () => {
  const subjects: Record<string, string> = { [A]: "Break the parser", [C]: "Tidy the lexer" };
  const describe_ = (oid: string) => subjects[oid] ?? null;

  it("names the commit under test and acts on HEAD", () => {
    const banner = bisectBanner(state({ bad: A, good: [B] }), describe_);
    expect(banner.title).toBe("Bisect in progress");
    expect(banner.detail).toContain("ccccccc “Tidy the lexer”");
    expect(banner.detail).toContain("main");
    expect(banner.actions).toEqual(["markGood", "markBad", "markSkip", "resetBisect"]);
  });

  it("leaves out the marks of a HEAD that is marked already", () => {
    expect(bisectBanner(state({ bad: C })).actions).toEqual(["resetBisect"]);
    expect(bisectBanner(state({ good: [C] })).actions).toEqual(["resetBisect"]);
    expect(bisectBanner(state({ bad: A })).actions).toContain("markGood");
  });

  it("says which mark is still missing", () => {
    expect(bisectBanner(state({ bad: A })).detail).toContain("good commit");
    expect(bisectBanner(state({ good: [B] })).detail).toContain("bad commit");
    expect(bisectBanner(state()).detail).toContain("bad commit and a good one");
  });

  it("speaks the terms the bisect was started with", () => {
    const own = state({ bad: A, terms: { bad: "broken", good: "fixed" } });
    expect(bisectBanner(own).detail).toContain("fixed commit");
  });

  it("names the first bad commit at the end and offers to show it and reset", () => {
    const banner = bisectBanner(state({ bad: A, good: [B], firstBad: A }), describe_);
    expect(banner.title).toBe("First bad commit found");
    expect(banner.detail).toContain("aaaaaaa “Break the parser” is the first bad commit");
    expect(banner.actions).toEqual(["showFirstBad", "resetBisect"]);
  });

  it("lists the candidates when only skipped commits are left", () => {
    const banner = bisectBanner(state({ bad: A, good: [B], skipped: [C], candidates: [C, A] }));
    expect(banner.detail).toContain("ccccccc, aaaaaaa");
    expect(banner.actions).toEqual(["resetBisect"]);
  });

  it("says a detached start by its short id", () => {
    expect(bisectBanner(state({ start: D, bad: A, good: [B] })).detail).toContain("ddddddd");
  });
});

describe("the end of the search", () => {
  it("is new once, for the repository it happened in", () => {
    const found = state({ bad: A, good: [B], firstBad: A });
    expect(newlyFound({ repo: "r", firstBad: null }, "r", found)).toBe(A);
    expect(newlyFound({ repo: "r", firstBad: A }, "r", found)).toBeNull();
    expect(newlyFound({ repo: "r", firstBad: null }, "r", state())).toBeNull();
  });

  // Opening or coming back to a repository that ended its search earlier does not take the
  // selection away: the banner has Show Commit for that.
  it("is not new when the repository only comes on screen", () => {
    const found = state({ bad: A, good: [B], firstBad: A });
    expect(newlyFound(null, "r", found)).toBeNull();
    expect(newlyFound({ repo: "other", firstBad: null }, "r", found)).toBeNull();
  });

  it("asks before Reset forgets the marks, not after the end", () => {
    expect(resetQuestion(state({ bad: A }))?.message).toContain("main is checked out again");
    expect(resetQuestion(state({ bad: A, good: [B], firstBad: A }))).toBeNull();
  });
});

describe("the graph's commit menu", () => {
  const facts = {
    isHeadCommit: false,
    detachedHere: false,
    onHead: true,
    published: false,
    parents: 1,
    upstream: "origin/main",
    hasRemote: true,
  };

  it("holds Bisect between the Reset group and Push Up To", () => {
    const labels = graphCommitMenu(facts, bisectCommitMenu({ kind: "clean" }, A)).map((row) =>
      row.separator ? "-" : row.label,
    );
    const at = labels.indexOf("Bisect");
    expect(labels.slice(at - 2, at + 3)).toEqual(["Roll Back Tree", "-", "Bisect", "-", "Push Up To"]);
  });
});
