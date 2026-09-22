import { describe, expect, it } from "vitest";
import type { Operation, OperationChanged, RepoId } from "$lib/ipc";
import {
  applyPending,
  confirmExitAfter,
  defaultAction,
  exitButtons,
  exitHeading,
  exitNote,
  exitRows,
  exitVariant,
  focusAfterSwitch,
  mustAskBeforeExit,
  progressPercent,
  seedPending,
  settleVerdict,
  showsDontShow,
  waitVerdict,
} from "./exit";

const repo = (id: number) => id as unknown as RepoId;

function op(over: Partial<Operation>): Operation {
  return {
    id: 1,
    repo: repo(1),
    kind: "push",
    label: "Pushing",
    phase: "running",
    success: null,
    ...over,
  };
}

function changed(over: Partial<OperationChanged>): OperationChanged {
  return {
    id: 1,
    repo: repo(1),
    kind: "push",
    label: "Pushing",
    phase: "running",
    success: null,
    ...over,
  };
}

const names = new Map([
  [1, "SignalGenerator200"],
  [2, "kors"],
]);
const idle = { repo: null, line: null };

describe("mustAskBeforeExit", () => {
  it("asks when the user wants to be asked", () => {
    expect(mustAskBeforeExit(true, 0, "window")).toBe(true);
    expect(mustAskBeforeExit(true, 0, "command")).toBe(true);
  });

  it("stays quiet when the user said not to ask and nothing is running", () => {
    expect(mustAskBeforeExit(false, 0, "window")).toBe(false);
  });

  it("asks anyway while something is running, whatever the setting says", () => {
    expect(mustAskBeforeExit(false, 1, "window")).toBe(true);
    expect(mustAskBeforeExit(false, 2, "command")).toBe(true);
  });

  it("lets the system end the session unasked when nothing would be lost", () => {
    expect(mustAskBeforeExit(true, 0, "system")).toBe(false);
  });

  it("still warns at shutdown while a push is running", () => {
    expect(mustAskBeforeExit(false, 1, "system")).toBe(true);
  });
});

describe("the two variants", () => {
  it("is the plain question with an empty queue and the warning otherwise", () => {
    expect(exitVariant(0)).toBe("plain");
    expect(exitVariant(1)).toBe("busy");
  });

  it("asks the question, or counts what is still running", () => {
    expect(exitHeading("plain", 0)).toBe("Do you want to exit Cogit now?");
    expect(exitHeading("busy", 1)).toBe("1 operation is still running");
    expect(exitHeading("busy", 3)).toBe("3 operations are still running");
  });

  it("offers the checkbox only on the question: the warning cannot be silenced", () => {
    expect(showsDontShow("plain")).toBe(true);
    expect(showsDontShow("busy")).toBe(false);
  });
});

describe("exitNote", () => {
  it("talks about the last window only when a window is what was closed", () => {
    expect(exitNote("window", "plain")).toBe("By closing the last window you will exit Cogit.");
  });

  it("says nothing about windows when the exit came from the menu or a shortcut", () => {
    expect(exitNote("command", "plain")).toBeNull();
    expect(exitNote("system", "plain")).toBeNull();
  });

  it("leaves the warning to its list", () => {
    expect(exitNote("window", "busy")).toBeNull();
  });
});

describe("buttons", () => {
  it("puts the decision rightmost, after Cancel", () => {
    expect(exitButtons("plain", false).map((button) => [button.label, button.tone])).toEqual([
      ["Cancel", "normal"],
      ["Exit Now", "primary"],
    ]);
  });

  it("offers waiting or stopping while work is running, stopping in the warning colour", () => {
    expect(exitButtons("busy", false).map((button) => [button.label, button.tone])).toEqual([
      ["Cancel", "normal"],
      ["Exit When Done", "normal"],
      ["Exit Anyway", "warning"],
    ]);
  });

  it("shows that it is already waiting and cannot be asked to wait twice", () => {
    const waiting = exitButtons("busy", true).find((button) => button.action === "exitWhenDone");
    expect(waiting).toMatchObject({ label: "Exiting When Done…", disabled: true });
  });

  it("defaults to exiting on the question and to cancelling on the warning", () => {
    expect(defaultAction("plain")).toBe("exit");
    // A stray Enter must never cut a push off half way.
    expect(defaultAction("busy")).toBe("cancel");
  });
});

describe("focusAfterSwitch", () => {
  it("keeps the focus where it was when that button is still there", () => {
    expect(focusAfterSwitch("cancel", "plain", false)).toBe("cancel");
    expect(focusAfterSwitch("cancel", "busy", false)).toBe("cancel");
  });

  it("moves off a button that went away, to the new default", () => {
    expect(focusAfterSwitch("exit", "busy", false)).toBe("cancel");
    expect(focusAfterSwitch("exitAnyway", "plain", false)).toBe("exit");
  });

  it("starts on the default when nothing had the focus", () => {
    expect(focusAfterSwitch(null, "plain", false)).toBe("exit");
    expect(focusAfterSwitch(null, "busy", false)).toBe("cancel");
  });

  it("does not leave the focus on a button that is disabled", () => {
    expect(focusAfterSwitch("exitWhenDone", "busy", true)).toBe("cancel");
  });
});

describe("confirmExitAfter", () => {
  it("stores a ticked box when the user goes through with the exit", () => {
    expect(confirmExitAfter("exit", "plain", true, true)).toBe(false);
  });

  it("stores an unticked box too, which is how the question comes back", () => {
    expect(confirmExitAfter("exit", "plain", false, false)).toBe(true);
  });

  it("does not touch the setting when nothing changed", () => {
    expect(confirmExitAfter("exit", "plain", false, true)).toBeNull();
  });

  it("forgets the box on Cancel: a user who stayed has not confirmed anything", () => {
    expect(confirmExitAfter("cancel", "plain", true, true)).toBeNull();
  });

  it("ignores a box that was not on screen when the choice was made", () => {
    expect(confirmExitAfter("exitAnyway", "busy", true, true)).toBeNull();
    expect(confirmExitAfter("exitWhenDone", "busy", true, true)).toBeNull();
  });
});

describe("progressPercent", () => {
  it("reads the percentage git prints", () => {
    expect(progressPercent("Writing objects:  45% (9/20), 1.20 MiB | 600.00 KiB/s")).toBe(45);
  });

  it("knows nothing before git said anything, or when the line has no number", () => {
    expect(progressPercent(null)).toBeUndefined();
    expect(progressPercent("Enumerating objects: 20, done.")).toBeUndefined();
  });
});

describe("exitRows", () => {
  it("names the kind, the repository and how far it got", () => {
    const rows = exitRows(
      [op({ id: 1 }), op({ id: 2, repo: repo(2), kind: "fetch", phase: "queued" })],
      names,
      { repo: repo(1), line: "Writing objects:  45% (9/20)" },
    );
    expect(rows).toEqual([
      { id: 1, text: "Push · SignalGenerator200 · 45%" },
      { id: 2, text: "Fetch · kors · waiting" },
    ]);
  });

  it("says running when git has not reported a percentage", () => {
    expect(exitRows([op({ kind: "commit" })], names, idle)).toEqual([
      { id: 1, text: "Commit · SignalGenerator200 · running" },
    ]);
  });

  it("does not lend one repository's progress to another", () => {
    const rows = exitRows([op({ repo: repo(2) })], names, { repo: repo(1), line: "45%" });
    expect(rows[0]?.text).toBe("Push · kors · running");
  });

  it("does not invent a repository for work that belongs to none", () => {
    expect(exitRows([op({ repo: null, kind: "other" })], names, idle)).toEqual([
      { id: 1, text: "Operation · running" },
    ]);
  });
});

describe("the live queue", () => {
  it("starts from the snapshot, minus whatever finished while it was being fetched", () => {
    const pending = seedPending(
      [op({ id: 1 }), op({ id: 2 }), op({ id: 3, phase: "done" })],
      [changed({ id: 2, phase: "done", success: true })],
    );
    expect([...pending.keys()]).toEqual([1]);
  });

  it("keeps what started while the snapshot was on its way", () => {
    const pending = seedPending([op({ id: 1 })], [changed({ id: 5, phase: "queued" })]);
    expect([...pending.keys()]).toEqual([1, 5]);
  });

  it("adds what starts and drops what finishes", () => {
    let pending = seedPending([op({ id: 1 })], []);
    pending = applyPending(pending, changed({ id: 2, phase: "queued" }));
    expect([...pending.keys()]).toEqual([1, 2]);
    pending = applyPending(pending, changed({ id: 1, phase: "done", success: true }));
    expect([...pending.keys()]).toEqual([2]);
  });

  it("follows a waiting operation into running", () => {
    let pending = seedPending([op({ id: 4, phase: "queued" })], []);
    pending = applyPending(pending, changed({ id: 4, phase: "running" }));
    expect(pending.get(4)?.phase).toBe("running");
  });
});

describe("settleVerdict", () => {
  it("leaves an undecided question on screen when the queue changes", () => {
    expect(settleVerdict("window", false, 0, false)).toBe("wait");
    expect(settleVerdict("command", false, 0, false)).toBe("wait");
  });

  it("lets a held shutdown go on once nothing is left to lose", () => {
    expect(settleVerdict("system", false, 0, false)).toBe("exit");
    expect(settleVerdict("system", false, 1, false)).toBe("wait");
  });

  it("follows Exit When Done once the user chose it", () => {
    expect(settleVerdict("window", true, 0, false)).toBe("exit");
    expect(settleVerdict("window", true, 0, true)).toBe("stay");
  });
});

describe("waitVerdict", () => {
  it("keeps waiting while anything is left", () => {
    expect(waitVerdict(1, false)).toBe("wait");
  });

  it("exits once the queue is empty", () => {
    expect(waitVerdict(0, false)).toBe("exit");
  });

  it("stays when something failed: the error is what the user has to see", () => {
    expect(waitVerdict(1, true)).toBe("stay");
    expect(waitVerdict(0, true)).toBe("stay");
  });
});
