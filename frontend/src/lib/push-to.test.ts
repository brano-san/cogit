import { describe, expect, it } from "vitest";
import {
  choosesRemote,
  customRefProblem,
  initialRemote,
  pushRefspec,
  menuPush,
  pushTitle,
  pushUpTo,
  tracksByDefault,
  splitUpstream,
  type PushSource,
} from "./push-to";

const remotes = ["origin", "team/mirror"];
const tracked: PushSource = { kind: "branch", name: "topic", upstream: "origin/feature/topic" };
const untracked: PushSource = { kind: "branch", name: "topic", upstream: null };
const tag: PushSource = { kind: "tag", name: "v1.0", upstream: null };

describe("splitUpstream", () => {
  it("splits at the remote, keeping slashes in the branch", () => {
    expect(splitUpstream("origin/feature/topic", remotes)).toEqual({
      remote: "origin",
      branch: "feature/topic",
    });
  });

  it("prefers the longest remote name, which may hold a slash", () => {
    expect(splitUpstream("team/mirror/main", remotes)).toEqual({ remote: "team/mirror", branch: "main" });
  });

  it("knows nothing of a remote that is not configured", () => {
    expect(splitUpstream("gone/main", remotes)).toBeNull();
  });
});

describe("initialRemote", () => {
  it("opens on the tracked remote, else on the primary one", () => {
    expect(initialRemote({ ...tracked, upstream: "team/mirror/x" }, remotes, "origin")).toBe("team/mirror");
    expect(initialRemote(untracked, remotes, "origin")).toBe("origin");
    expect(initialRemote(untracked, [], null)).toBeNull();
  });
});

describe("pushRefspec", () => {
  it("pushes a branch to the branch it tracks on that remote", () => {
    expect(pushRefspec(tracked, { mode: "tracked" }, "origin", remotes)).toBe(
      "refs/heads/topic:refs/heads/feature/topic",
    );
  });

  it("pushes to the same name on a remote the branch does not track", () => {
    expect(pushRefspec(tracked, { mode: "tracked" }, "team/mirror", remotes)).toBe(
      "refs/heads/topic:refs/heads/topic",
    );
    expect(pushRefspec(untracked, { mode: "tracked" }, "origin", remotes)).toBe(
      "refs/heads/topic:refs/heads/topic",
    );
  });

  it("puts a short custom ref under refs/heads and keeps a full one as written", () => {
    expect(pushRefspec(untracked, { mode: "custom", ref: "review/topic" }, "origin", remotes)).toBe(
      "refs/heads/topic:refs/heads/review/topic",
    );
    expect(pushRefspec(untracked, { mode: "custom", ref: "refs/for/main" }, "origin", remotes)).toBe(
      "refs/heads/topic:refs/for/main",
    );
  });

  it("pushes a tag as a tag", () => {
    expect(pushRefspec(tag, { mode: "tracked" }, "origin", remotes)).toBe("refs/tags/v1.0:refs/tags/v1.0");
    expect(pushRefspec(tag, { mode: "custom", ref: "release" }, "origin", remotes)).toBe(
      "refs/tags/v1.0:refs/tags/release",
    );
  });
});

describe("pushUpTo", () => {
  it("pushes the commit onto the upstream branch", () => {
    expect(pushUpTo("abc123", "origin/main", remotes)).toEqual({
      remote: "origin",
      refspec: "abc123:refs/heads/main",
    });
    expect(pushUpTo("abc123", "gone/main", remotes)).toBeNull();
  });
});

describe("customRefProblem", () => {
  it("asks for a name and catches what Git would refuse", () => {
    expect(customRefProblem("  ")).toMatch(/Enter/);
    for (const bad of ["two..dots", "has space", "end/", "-dash", "x.lock", "a:b"]) {
      expect(customRefProblem(bad), bad).not.toBeNull();
    }
    expect(customRefProblem("review/topic")).toBeNull();
  });
});

describe("pushTitle", () => {
  it("names the ref and the remote the way SmartGit does", () => {
    expect(pushTitle(tracked, "origin")).toBe("Push 'topic' to remote 'origin'");
  });
});

// Push in a branch's menu sent refs/heads/x:refs/heads/x without --set-upstream: the
// graph kept `x` and `origin/x` apart, since the branch still tracked nothing.
describe("menuPush", () => {
  it("makes a branch never pushed track what it becomes", () => {
    expect(menuPush(untracked, ["origin"], "origin")).toEqual({
      remote: "origin",
      refspec: "refs/heads/topic:refs/heads/topic",
      track: true,
    });
  });

  it("leaves a tracking branch and a tag as they are", () => {
    expect(menuPush(tracked, remotes, "origin")).toEqual({
      remote: "origin",
      refspec: "refs/heads/topic:refs/heads/feature/topic",
      track: false,
    });
    expect(menuPush(tag, remotes, "origin")?.track).toBe(false);
  });

  it("has nowhere to push without a remote", () => {
    expect(menuPush(untracked, [], null)).toBeNull();
  });
});

describe("tracksByDefault", () => {
  it("ticks Set Upstream in Push To only for a branch that tracks nothing yet", () => {
    expect(tracksByDefault(untracked)).toBe(true);
    expect(tracksByDefault(tracked)).toBe(false);
    expect(tracksByDefault(tag)).toBe(false);
  });
});

// Push of a branch never pushed went to origin whatever the other remotes were.
describe("choosesRemote", () => {
  it("asks where to publish a new branch when more than one remote could take it", () => {
    expect(choosesRemote(untracked, remotes)).toBe(true);
    expect(choosesRemote(untracked, ["origin"])).toBe(false);
    expect(choosesRemote(tracked, remotes)).toBe(false);
    expect(choosesRemote(tag, remotes)).toBe(false);
  });
});
