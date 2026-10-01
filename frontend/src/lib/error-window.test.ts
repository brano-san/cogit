import { describe, expect, it } from "vitest";
import { entryOf, pushEntry, repoNameOf, startsDrag, titleOf, type ErrorEntry } from "./error-window";

const run = (id: number, over: Partial<Parameters<typeof entryOf>[0]> = {}) => ({
  id,
  repo: "C:/repos/cogit",
  operation: "Push",
  command: "git push origin master",
  summary: "error: failed to push some refs",
  severity: "failure" as const,
  stoppedOnConflicts: false,
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
