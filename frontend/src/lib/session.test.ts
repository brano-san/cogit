import { describe, expect, it } from "vitest";
import { readSession, writeSession, type Session } from "./session";

const full: Session = {
  repositories: ["C:/work/one", "C:/work/two"],
  active: "C:/work/two",
  selected: { "C:/work/two": "abc123" },
};

describe("readSession", () => {
  it("reads back what was written", () => {
    expect(readSession(JSON.parse(writeSession(full)))).toEqual(full);
  });

  it("gives an empty session for nothing stored", () => {
    expect(readSession(null)).toEqual({ repositories: [], active: null, selected: {} });
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
    const empty: Session = { repositories: [], active: null, selected: {} };
    expect(readSession(JSON.parse(writeSession(empty)))).toEqual(empty);
  });
});
