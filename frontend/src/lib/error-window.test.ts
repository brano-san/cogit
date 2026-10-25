import { describe, expect, it } from "vitest";
import { entryOf, outputToShow, pushEntry, repoNameOf, startsDrag, titleOf, windowTitle, isNextStep, type ErrorEntry } from "./error-window";

const run = (id: number, over: Partial<Parameters<typeof entryOf>[0]> = {}) => ({
  id,
  repo: "C:/repos/cogit",
  operation: "Push",
  command: "git push origin master",
  summary: "error: failed to push some refs",
  severity: "failure" as const,
  stoppedOnConflicts: false,
  expected: null,
  ...over,
});

describe("titleOf", () => {
  it("says what failed", () => {
    expect(titleOf(run(1))).toBe("Push failed");
  });

  it("calls a stash that stopped on conflicts applied, not failed", () => {
    const stash = run(1, {
      operation: "Stash",
      command: "git stash apply --index stash@{0}",
      stoppedOnConflicts: true,
    });
    expect(titleOf(stash)).toBe("Stash applied with conflicts");
    expect(titleOf({ ...stash, command: "git stash pop stash@{0}" })).toBe(
      "Stash applied with conflicts",
    );
  });

  it("says a merge or a rebase stopped", () => {
    expect(titleOf(run(1, { operation: "Merge", stoppedOnConflicts: true }))).toBe(
      "Merge stopped with conflicts",
    );
    expect(titleOf(run(1, { operation: "Rebase", stoppedOnConflicts: true }))).toBe(
      "Rebase stopped with conflicts",
    );
  });

  // R-841: a diverged Pull rebases, and its stop is the rebase's, not a failed pull.
  it("calls a pull that stopped mid-rebase a rebase, and a merging pull a pull", () => {
    const pull = { operation: "Pull (rebase)", stoppedOnConflicts: true };
    expect(titleOf(run(1, { ...pull, command: "git pull --progress --prune --rebase origin" }))).toBe(
      "Rebase stopped with conflicts",
    );
    expect(
      titleOf(run(1, { ...pull, operation: "Pull", command: "git pull --progress --prune --no-rebase origin" })),
    ).toBe("Pull stopped with conflicts");
  });
});

// R-840: an ordinary state of things is a warning with the whole output, never red.
describe("an expected failure", () => {
  it("is a warning about a gone upstream, with nothing to offer", () => {
    const entry = entryOf(run(1, { operation: "Pull", command: "git pull --ff-only origin", expected: "upstreamGone" }));
    expect(entry).toMatchObject({ kind: "warning", title: "Upstream branch is gone", offer: null });
  });

  it("is a warning about a clone kept without its submodules (R-870)", () => {
    const entry = entryOf(run(1, { operation: "Clone", command: "git clone --recurse-submodules", expected: "submodulesFailed" }));
    expect(entry).toMatchObject({ kind: "warning", title: "Submodules not cloned", offer: null });
  });

  it("offers Pull for a push the remote refused as behind", () => {
    const entry = entryOf(run(1, { expected: "pushBehind" }));
    expect(entry).toMatchObject({ kind: "warning", title: "Push rejected", offer: "pull" });
  });

  it("offers the conflicts of a command that stopped on them, and nothing for an error", () => {
    expect(entryOf(run(1, { stoppedOnConflicts: true })).offer).toBe("showConflicts");
    expect(entryOf(run(1)).offer).toBeNull();
  });
});

describe("isNextStep", () => {
  it("is a Continue or Skip that stopped on the next step's conflicts, not a first stop", () => {
    expect(isNextStep(run(1, { command: "git rebase --continue", stoppedOnConflicts: true }))).toBe(true);
    expect(isNextStep(run(1, { command: "git cherry-pick --skip", stoppedOnConflicts: true }))).toBe(true);
    expect(isNextStep(run(1, { command: "git rebase --continue" }))).toBe(false);
    expect(isNextStep(run(1, { command: "git rebase main", stoppedOnConflicts: true }))).toBe(false);
  });
});

describe("entryOf", () => {
  it("is a warning only when the engine saw the command stop on conflicts", () => {
    expect(entryOf(run(1)).kind).toBe("error");
    expect(entryOf(run(1, { stoppedOnConflicts: true })).kind).toBe("warning");
  });

  it("carries the operation's own repository and command", () => {
    const entry = entryOf(run(4));
    expect(entry.repo).toBe("C:/repos/cogit");
    expect(entry.command).toBe("git push origin master");
    expect(repoNameOf(entry.repo)).toBe("cogit");
  });
});

describe("pushEntry", () => {
  const at = (entries: ErrorEntry[]) => entries.map((entry) => `${entry.kind}:${entry.id}`);

  it("puts the newest first and every error ahead of every warning", () => {
    let list: ErrorEntry[] = [];
    list = pushEntry(list, entryOf(run(1, { stoppedOnConflicts: true, operation: "Merge" })));
    list = pushEntry(list, entryOf(run(2, { summary: "a" })));
    list = pushEntry(list, entryOf(run(3, { summary: "b" })));
    expect(at(list)).toEqual(["error:3", "error:2", "warning:1"]);
  });

  it("queues one record once", () => {
    const entry = entryOf(run(1));
    expect(pushEntry(pushEntry([], entry), entry)).toHaveLength(1);
  });

  it("counts the same failure again instead of growing the list", () => {
    let list = pushEntry([], entryOf(run(1)));
    list = pushEntry(list, entryOf(run(2)));
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ id: 2, repeats: 2 });
  });
});

describe("outputToShow", () => {
  const out = (id: number) => ({ id }) as never;

  // A made-up record ("did not start", 0 ms, started now) was shown when the journal had
  // dropped the command, and Copy output put it into bug reports.
  it("says the output is gone rather than inventing one", () => {
    expect(outputToShow(null, undefined)).toBe("gone");
  });

  it("keeps the output already received when the journal has since lost it", () => {
    expect(outputToShow(null, out(1))).toEqual(out(1));
  });

  it("prefers what the journal has now", () => {
    expect(outputToShow(out(2), out(1))).toEqual(out(2));
  });
});

describe("startsDrag", () => {
  const target = (inButton: boolean) => ({ closest: (selector: string) => (inButton && selector === "button" ? {} : null) });

  // Pointer capture on the header sends the click to the header, so a button inside it never
  // saw one: the cross of the window did nothing.
  it("is not a drag when the press is on a button", () => {
    expect(startsDrag({ button: 0, target: target(true) })).toBe(false);
  });

  it("is a drag with the main button on the bare header", () => {
    expect(startsDrag({ button: 0, target: target(false) })).toBe(true);
    expect(startsDrag({ button: 2, target: target(false) })).toBe(false);
  });
});

describe("windowTitle", () => {
  it("names warnings as warnings and errors as errors", () => {
    expect(windowTitle([{ kind: "warning" }])).toBe("Warning — Cogit");
    expect(windowTitle([{ kind: "warning" }, { kind: "warning" }])).toBe("Warnings — Cogit");
    expect(windowTitle([{ kind: "error" }])).toBe("Errors — Cogit");
    expect(windowTitle([{ kind: "error" }, { kind: "warning" }])).toBe("Errors and Warnings — Cogit");
  });
});
