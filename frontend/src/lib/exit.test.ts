import { describe, expect, it } from "vitest";
import type { Operation, RepoId } from "$lib/ipc";
import { exitBlockers, mustAskBeforeExit } from "./exit";

const repo = (id: number) => id as unknown as RepoId;

function op(over: Partial<Operation>): Operation {
  return {
    id: 1,
    repo: repo(1),
    kind: "push",
    label: "Push",
    phase: "running",
    success: null,
    ...over,
  };
}

const names = new Map([[1, "dtv_device"], [2, "kors"]]);

describe("exitBlockers", () => {
  it("has nothing to say when the queue is empty", () => {
    expect(exitBlockers([], names)).toEqual([]);
  });

  it("names each unfinished operation with its repository and whether it has started", () => {
    const lines = exitBlockers(
      [
        op({ id: 1, label: "Push", phase: "running" }),
        op({ id: 2, repo: repo(2), kind: "fetch", label: "Fetch", phase: "queued" }),
      ],
      names,
    );
    expect(lines).toEqual(["Push — dtv_device (running)", "Fetch — kors (waiting)"]);
  });

  it("does not count what has already finished", () => {
    expect(exitBlockers([op({ phase: "done", success: true })], names)).toEqual([]);
  });

  it("names work that belongs to no repository without inventing one", () => {
    expect(exitBlockers([op({ repo: null, label: "Fetch All" })], names)).toEqual([
      "Fetch All (running)",
    ]);
  });
});

describe("mustAskBeforeExit", () => {
  it("asks when the user wants to be asked", () => {
    expect(mustAskBeforeExit(true, 0)).toBe(true);
  });

  it("stays quiet when the user said not to ask and nothing is running", () => {
    expect(mustAskBeforeExit(false, 0)).toBe(false);
  });

  // Requirement 1.3: leaving in the middle of a push is never silent.
  it("asks anyway while something is running, whatever the setting says", () => {
    expect(mustAskBeforeExit(false, 1)).toBe(true);
  });
});
