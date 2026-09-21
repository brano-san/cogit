import { describe, expect, it } from "vitest";
import { forget, readSession, remember, writeSession, type Session } from "./session";

const full: Session = {
  recent: ["C:/work/two", "C:/work/one"],
  repositories: ["C:/work/one", "C:/work/two"],
  active: "C:/work/two",
  selected: { "C:/work/two": "abc123" },
};

describe("readSession", () => {
  it("reads back what was written", () => {
    expect(readSession(JSON.parse(writeSession(full)))).toEqual(full);
  });

  it("gives an empty session for nothing stored", () => {
    expect(readSession(null)).toEqual({ repositories: [], active: null, selected: {}, recent: [] });
  });

  it("gives an empty session for something that is not a session", () => {
    expect(readSession(42).repositories).toEqual([]);
  });

  it("drops entries that are not paths", () => {
    expect(readSession({ repositories: ["ok", 7, null] }).repositories).toEqual(["ok"]);
  });

  it("drops an active repository that is not in the list", () => {
    // It was closed in another window, or the folder is gone; opening it would fail.
    expect(readSession({ repositories: ["a"], active: "b" }).active).toBeNull();
  });

  it("keeps an active repository that is in the list", () => {
    expect(readSession({ repositories: ["a"], active: "a" }).active).toBe("a");
  });

  it("drops a selected commit whose repository is gone", () => {
    const session = readSession({ repositories: ["a"], selected: { a: "1", b: "2" } });
    expect(session.selected).toEqual({ a: "1" });
  });

  it("drops a selection that is not an object id", () => {
    expect(readSession({ repositories: ["a"], selected: { a: 5 } }).selected).toEqual({});
  });

  it("keeps the order the repositories were opened in", () => {
    expect(readSession({ repositories: ["b", "a"] }).repositories).toEqual(["b", "a"]);
  });

  it("drops a repeated path so it is not opened twice", () => {
    expect(readSession({ repositories: ["a", "a"] }).repositories).toEqual(["a"]);
  });
});

describe("writeSession", () => {
  it("writes json a reader can take back", () => {
    expect(() => JSON.parse(writeSession(full))).not.toThrow();
  });

  it("writes an empty session without complaining", () => {
    const empty: Session = { repositories: [], active: null, selected: {}, recent: [] };
    expect(readSession(JSON.parse(writeSession(empty)))).toEqual(empty);
  });
});

describe("recent repositories", () => {
  it("are empty to begin with", () => {
    expect(readSession(null).recent).toEqual([]);
  });

  it("survive a repository being closed", () => {
    const session = readSession({ repositories: [], recent: ["C:/gone"] });
    expect(session.recent).toEqual(["C:/gone"]);
  });

  it("keep the newest first", () => {
    expect(remember([], "C:/a")).toEqual(["C:/a"]);
    expect(remember(["C:/a"], "C:/b")).toEqual(["C:/b", "C:/a"]);
  });

  it("move a path already known to the front instead of repeating it", () => {
    expect(remember(["C:/a", "C:/b"], "C:/b")).toEqual(["C:/b", "C:/a"]);
  });

  it("stop at a sensible length", () => {
    const many = Array.from({ length: 40 }, (_, n) => `C:/r${n}`);
    expect(remember(many, "C:/new").length).toBeLessThanOrEqual(20);
  });

  it("drop a path the user asked to forget", () => {
    expect(forget(["C:/a", "C:/b"], "C:/a")).toEqual(["C:/b"]);
  });

  it("drop entries that are not paths when read back", () => {
    expect(readSession({ recent: ["ok", 7] }).recent).toEqual(["ok"]);
  });
});

describe("writing often", () => {
  it("validates on the way in, so a hot path does not pay for it twice", () => {
    // readSession is the gate for what comes off disk; a write of an already-valid
    // session must not need it again.
    const twice = readSession(readSession(JSON.parse(writeSession(full))));
    expect(twice).toEqual(full);
  });
});
