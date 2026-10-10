import { describe, expect, it } from "vitest";
import type { PushPreview } from "$lib/ipc/network-dialogs";
import type { PushChoice } from "./network-dialogs";
import { effectiveChoice, fetchedText, pushCommand, pushStatus, upstreamState } from "./push-dialog";

const choice = (over: Partial<PushChoice> = {}): PushChoice => ({
  remote: "origin",
  local: "main",
  branch: "main",
  setUpstream: true,
  tags: "none",
  notes: false,
  forceWithLease: false,
  ...over,
});

describe("Set as upstream", () => {
  it("says the branch already tracks the target and sends no --set-upstream", () => {
    expect(upstreamState("origin/main", "origin", "main")).toEqual({
      kind: "already",
      hint: "Already tracking origin/main",
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
    expect(pushCommand(choice({ setUpstream: false }))).toEqual({
      short: "git push origin main",
      full: "git push --progress origin refs/heads/main:refs/heads/main",
    });
  });

  it("names both branches when they differ", () => {
    expect(pushCommand(choice({ setUpstream: false, local: "feature/x", branch: "release" })).short).toBe(
      "git push origin feature/x:release",
    );
  });

  it("carries every option, the notes as a second command", () => {
    expect(pushCommand(choice({ tags: "all", notes: true, forceWithLease: true }))).toEqual({
      short:
        "git push --set-upstream --tags --force-with-lease --force-if-includes origin main; git push origin refs/notes/*:refs/notes/*",
      full: "git push --progress --set-upstream --tags --force-with-lease --force-if-includes origin refs/heads/main:refs/heads/main; git push --progress origin refs/notes/*:refs/notes/*",
    });
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
});
