import { describe, expect, it, vi } from "vitest";
import { resolveClose, saveFlow, toolFollowUp, toolPrompt } from "./solver-flow";

function deps(unresolved: number, confirm = true) {
  const calls: string[] = [];
  return {
    calls,
    steps: {
      unresolved,
      confirmMarkers: vi.fn(async () => {
        calls.push("confirm");
        return confirm;
      }),
      write: vi.fn(async () => void calls.push("write")),
      announce: vi.fn(async () => void calls.push("announce")),
      saved: vi.fn(() => void calls.push("saved")),
      close: vi.fn(async () => void calls.push("close")),
    },
  };
}

describe("save", () => {
  it("writes, announces and closes when every conflict is resolved, without asking", async () => {
    const { calls, steps } = deps(0);
    expect(await saveFlow(steps)).toEqual({ kind: "saved" });
    expect(calls).toEqual(["write", "announce", "saved", "close"]);
  });

  it("asks before writing conflict markers, and writes them on a yes", async () => {
    const { calls, steps } = deps(2);
    expect(await saveFlow(steps)).toEqual({ kind: "saved" });
    expect(calls).toEqual(["confirm", "write", "announce", "saved", "close"]);
  });

  it("writes nothing on a no", async () => {
    const { calls, steps } = deps(2, false);
    expect(await saveFlow(steps)).toEqual({ kind: "canceled" });
    expect(calls).toEqual(["confirm"]);
  });

  it("keeps the window and says why when the write fails", async () => {
    const { calls, steps } = deps(0);
    steps.write.mockRejectedValueOnce(new Error("locked"));
    const result = await saveFlow(steps);
    expect(result.kind).toBe("failed");
    expect(calls).not.toContain("close");
    expect(calls).not.toContain("saved");
  });
});

describe("closing with edits", () => {
  it("asks once on close: resolve, keep or stay, the edits folded in", () => {
    expect(resolveClose(false, 0)).toEqual({
      choices: [
        { choice: "cancel", label: "Cancel" },
        { choice: "keep", label: "Keep Unresolved" },
        { choice: "resolve", label: "Mark Resolved" },
      ],
      primary: "resolve",
      warning: null,
    });
    const dirty = resolveClose(true, 0);
    expect(dirty.choices.map((each) => each.choice)).toEqual(["cancel", "discard", "keep", "resolve"]);
    expect(dirty.choices.at(-1)?.label).toBe("Save and Mark Resolved");
  });

  it("warns about markers and makes Keep Unresolved the default", () => {
    const marked = resolveClose(true, 2);
    expect(marked.primary).toBe("keep");
    expect(marked.warning).toMatch(/2 conflicts are still undecided/);
  });
});

describe("after the external tool", () => {
  const done = { exitCode: 0, canceled: false, markersLeft: false, conflicted: true };

  it("offers to mark the file resolved when no marker is left", () => {
    expect(toolFollowUp(done)).toBe("offerResolve");
  });

  it("leaves the file conflicted while markers remain", () => {
    expect(toolFollowUp({ ...done, markersLeft: true })).toBe("stillConflicted");
  });

  it("a canceled tool changes nothing", () => {
    expect(toolFollowUp({ ...done, canceled: true })).toBe("canceled");
  });

  it("a file already resolved some other way needs nothing", () => {
    expect(toolFollowUp({ ...done, conflicted: false })).toBe("alreadyResolved");
  });

  it("the question names a failing exit code", () => {
    expect(toolPrompt("a.txt", done)).not.toContain("exit code");
    expect(toolPrompt("a.txt", { ...done, exitCode: 2 })).toContain("exit code 2");
  });
});
