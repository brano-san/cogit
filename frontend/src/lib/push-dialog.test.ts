import { describe, expect, it } from "vitest";
import { shortOid } from "$lib/format";
import type { PushPreview } from "$lib/ipc/network-dialogs";
import type { PushChoice } from "./network-dialogs";
import {
  commitsText,
  effectiveChoice,
  fetchedText,
  leaseLine,
  protectedNames,
  pushButton,
  pushCommand,
  pushStatus,
  staleChoices,
  statusLine,
  notesNote,
  tagCounter,
  upstreamState,
  type TagCount,
} from "./push-dialog";

const choice = (over: Partial<PushChoice> = {}): PushChoice => ({
  remote: "origin",
  local: "main",
  branch: "main",
  setUpstream: true,
  tags: "none",
  notes: false,
  forceWithLease: false,
  forceIfIncludes: true,
  noVerify: false,
  dryRun: false,
  atomic: false,
  ...over,
});

describe("Set as upstream", () => {
  it("says the branch already tracks the target and sends no --set-upstream", () => {
    expect(upstreamState("origin/main", "origin", "main")).toEqual({
      kind: "already",
      hint: "Upstream: origin/main",
    });
    expect(effectiveChoice(choice(), "origin/main").setUpstream).toBe(false);
  });

  it("names the upstream it replaces", () => {
    expect(upstreamState("origin/old", "origin", "main").hint).toBe("Track origin/main instead of origin/old");
    expect(effectiveChoice(choice(), "origin/old").setUpstream).toBe(true);
  });

  it("says when there is no upstream yet, slashes and all", () => {
    expect(upstreamState(null, "fork", "feature/x").hint).toBe("Track fork/feature/x (no upstream yet)");
    expect(upstreamState("fork/feature/x", "fork", "feature/x").kind).toBe("already");
  });
});

describe("the command", () => {
  it("is short as typed and full as run", () => {
    expect(pushCommand(choice({ setUpstream: false }), null)).toEqual({
      short: "git push origin main",
      full: "git push --progress origin refs/heads/main:refs/heads/main",
    });
  });

  it("names both branches when they differ", () => {
    expect(pushCommand(choice({ setUpstream: false, local: "feature/x", branch: "release" }), null).short).toBe(
      "git push origin feature/x:release",
    );
  });

  it("carries every option in one command, the notes refspec quoted for the shell", () => {
    expect(pushCommand(choice({ tags: "all", notes: true, forceWithLease: true }), null)).toEqual({
      short: "git push --set-upstream --tags --force-with-lease=main --force-if-includes origin main 'refs/notes/*:refs/notes/*'",
      full: "git push --progress --set-upstream --tags --force-with-lease=refs/heads/main --force-if-includes origin refs/heads/main:refs/heads/main 'refs/notes/*:refs/notes/*'",
    });
  });
});

describe("force", () => {
  it("writes the expected tip into the lease without the includes check", () => {
    const tip = "1234567890abcdef1234567890abcdef12345678";
    const explicit = choice({ setUpstream: false, branch: "feature/x", local: "feature/x", forceWithLease: true, forceIfIncludes: false });
    expect(pushCommand(explicit, tip)).toEqual({
      short: `git push --force-with-lease=feature/x:${tip} origin feature/x`,
      full: `git push --progress --force-with-lease=refs/heads/feature/x:${tip} origin refs/heads/feature/x:refs/heads/feature/x`,
    });
    expect(pushCommand(explicit, null).short).toBe("git push --force-with-lease=feature/x: origin feature/x");
  });

  it("shows what the lease expects", () => {
    const tip = "1234567890abcdef1234567890abcdef12345678";
    expect(leaseLine("origin", "main", tip)).toEqual({
      flag: `--force-with-lease=main:${shortOid(tip)}`,
      note: "origin/main as last fetched",
    });
    expect(leaseLine("origin", "new", null)).toEqual({
      flag: "--force-with-lease=new:",
      note: "new must not exist on origin",
    });
  });

  it("reads the protected names a person wrote", () => {
    expect(protectedNames(" main, release/1 ,main  dev")).toEqual(["main", "release/1", "dev"]);
    expect(protectedNames("")).toEqual([]);
  });
});

const preview = (over: Partial<PushPreview> = {}): PushPreview => ({
  total: 0,
  commits: [],
  behind: 0,
  localTip: "a",
  remoteTip: "a",
  fetchedAt: null,
  notesUnpushed: null,
  hasLocalNotes: false,
  ...over,
});

describe("the status line", () => {
  it("says how the branch stands against the remote-tracking one", () => {
    expect(pushStatus(preview({ total: 2, localTip: "b" }), "origin")).toBe("2 ahead");
    expect(pushStatus(preview({ behind: 3, remoteTip: "c" }), "origin")).toBe("3 behind");
    expect(pushStatus(preview({ total: 2, behind: 3, localTip: "b", remoteTip: "c" }), "origin")).toBe(
      "2 ahead, 3 behind — diverged",
    );
    expect(pushStatus(preview(), "origin")).toBe("Up to date");
    expect(pushStatus(preview({ remoteTip: null, total: 4 }), "fork")).toBe("New branch on fork");
  });

  it("dates the last fetch", () => {
    expect(fetchedText(null, 0)).toBe("never fetched");
    expect(fetchedText(0, 5 * 60_000)).toBe("fetched 5 minutes ago");
    expect(fetchedText(1_000, 1_000)).toBe("fetched just now");
  });

  const HOUR = 3_600_000;

  it("puts the status and the fetch time on one line while the fetch is recent", () => {
    expect(statusLine(preview({ total: 2, localTip: "b", fetchedAt: 0 }), "origin", 5 * 60_000, 24)).toEqual({
      text: "2 ahead · fetched 5 minutes ago",
      stale: false,
    });
    expect(statusLine(preview({ fetchedAt: 0 }), "origin", 24 * HOUR, 24).stale).toBe(false);
  });

  it("qualifies the status past the threshold, and when nothing was ever fetched", () => {
    expect(statusLine(preview({ fetchedAt: 0 }), "origin", 22 * 24 * HOUR, 24)).toEqual({
      text: "Up to date as of last fetch (22 days ago)",
      stale: true,
    });
    expect(statusLine(preview({ fetchedAt: 0 }), "origin", 2 * HOUR, 1).stale).toBe(true);
    expect(statusLine(preview(), "origin", 0, 24)).toEqual({ text: "Up to date · never fetched", stale: true });
  });

  it("offers thresholds from an hour to a month, and keeps one written by hand", () => {
    expect(staleChoices(24)).toContainEqual(["24", "1 day"]);
    expect(staleChoices(24).map(([hours]) => hours)).toEqual(["1", "6", "12", "24", "72", "168", "720"]);
    expect(staleChoices(48)).toContainEqual(["48", "48 hours"]);
    expect(staleChoices(48).map(([hours]) => Number(hours))).toEqual([1, 6, 12, 24, 48, 72, 168, 720]);
  });
});

describe("counts", () => {
  it("counts the tags that would go beside the choice, their names or the failure in the tooltip", () => {
    expect(tagCounter(null)).toBeNull();
    expect(tagCounter({ state: "counting" })).toEqual({ text: "· counting…" });
    expect(tagCounter({ state: "done", tags: [] })).toEqual({ text: "· 0 to push" });
    expect(tagCounter({ state: "done", tags: ["v1", "v2"] })).toEqual({ text: "· 2 to push", tip: "v1\nv2" });
    expect(tagCounter({ state: "failed", reason: "no network" })).toEqual({
      text: "· could not count",
      tip: "Could not count tags: no network",
    });
  });

  it("says beside Push notes what is not on the remote yet", () => {
    expect(notesNote(preview({ hasLocalNotes: false }))).toBeUndefined();
    expect(notesNote(preview({ hasLocalNotes: true, notesUnpushed: 0 }))).toBeUndefined();
    expect(notesNote(preview({ hasLocalNotes: true, notesUnpushed: 3 }))).toBe("· 3 not pushed");
    expect(notesNote(preview({ hasLocalNotes: true, notesUnpushed: null }))).toBe("· not compared with the remote");
    expect(notesNote(null)).toBeUndefined();
  });

  it("says how many commits would go", () => {
    expect(commitsText(null)).toBe("Counting commits…");
    expect(commitsText(preview())).toBe("No commits to push");
    expect(commitsText(preview({ total: 1 }))).toBe("1 commit to push");
    expect(commitsText(preview({ total: 3 }))).toBe("3 commits to push");
  });
});

describe("the Push button", () => {
  const done = (names: string[]): TagCount => ({ state: "done", tags: names });
  const off = (over: Partial<PushChoice> = {}) => choice({ setUpstream: false, ...over });
  const button = (p: PushPreview | null, c: PushChoice, upstream: string | null = "origin/main", tags: TagCount | null = null) =>
    pushButton(p, c, upstream, tags);

  it("counts the commits when only they go, the upstream riding along", () => {
    expect(button(preview({ total: 1, localTip: "b" }), off())).toEqual({ label: "Push 1 commit", nothing: false });
    expect(button(preview({ total: 3, localTip: "b" }), off()).label).toBe("Push 3 commits");
    expect(button(preview({ total: 3, localTip: "b" }), choice({ setUpstream: true }), null).label).toBe("Push 3 commits");
  });

  it("names tags, notes or the upstream when that is all that goes", () => {
    expect(button(preview(), off({ tags: "follow" }), "origin/main", done(["v1"]))).toEqual({ label: "Push tags", nothing: false });
    expect(button(preview({ hasLocalNotes: true, notesUnpushed: 2 }), off({ notes: true }))).toEqual({ label: "Push notes", nothing: false });
    expect(button(preview(), choice({ setUpstream: true }), null)).toEqual({ label: "Set upstream", nothing: false });
  });

  it("is plain Push for a mix, a new branch, or what is not counted yet", () => {
    expect(button(preview({ total: 2, localTip: "b" }), off({ tags: "all" }), "origin/main", done(["v1"])).label).toBe("Push");
    expect(button(preview({ remoteTip: null }), off(), null)).toEqual({ label: "Push", nothing: false });
    expect(button(preview(), off({ tags: "all" }), "origin/main", { state: "counting" })).toEqual({ label: "Push", nothing: false });
    expect(button(preview(), off({ tags: "follow" }), "origin/main", { state: "failed", reason: "x" }).nothing).toBe(false);
    expect(button(preview({ hasLocalNotes: true, notesUnpushed: null }), off({ notes: true })).nothing).toBe(false);
    expect(button(null, off()).nothing).toBe(false);
  });

  it("has nothing to send when the remote has it all and nothing rides along", () => {
    expect(button(preview(), choice({ setUpstream: true }))).toEqual({ label: "Push", nothing: true });
    expect(button(preview(), off({ tags: "follow" }), "origin/main", done([])).nothing).toBe(true);
    expect(button(preview({ hasLocalNotes: true, notesUnpushed: 0 }), off({ notes: true })).nothing).toBe(true);
    expect(button(preview({ hasLocalNotes: false }), off({ notes: true })).nothing).toBe(true);
    // Only behind: without force the remote refuses to move back.
    expect(button(preview({ behind: 2, remoteTip: "c" }), off()).nothing).toBe(true);
  });

  it("says Force push for a forced move, even back", () => {
    expect(button(preview({ total: 2, localTip: "b", remoteTip: "c" }), off({ forceWithLease: true })).label).toBe("Force push 2 commits");
    expect(button(preview({ behind: 2, remoteTip: "c" }), off({ forceWithLease: true }))).toEqual({ label: "Force push", nothing: false });
  });

  it("calls a dry run Dry run", () => {
    expect(button(preview({ total: 3, localTip: "b" }), off({ dryRun: true, forceWithLease: true })).label).toBe("Dry run");
  });
});

describe("advanced options", () => {
  it("puts them in the command in the engine's order", () => {
    expect(pushCommand(choice({ setUpstream: false, noVerify: true, atomic: true }), null)).toEqual({
      short: "git push --no-verify --atomic origin main",
      full: "git push --progress --no-verify --atomic origin refs/heads/main:refs/heads/main",
    });
  });

  it("runs a dry run as one command, the notes in it", () => {
    expect(pushCommand(choice({ setUpstream: false, dryRun: true, notes: true }), null)).toEqual({
      short: "git push --dry-run origin main 'refs/notes/*:refs/notes/*'",
      full: "git push --dry-run --progress origin refs/heads/main:refs/heads/main 'refs/notes/*:refs/notes/*'",
    });
  });
});

