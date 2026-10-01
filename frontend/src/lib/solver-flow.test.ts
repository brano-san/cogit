import { describe, expect, it, vi } from "vitest";
import { afterCloseChoice, saveFlow, toolFollowUp, toolPrompt } from "./solver-flow";

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
  it("Discard lets the window close; Save and Cancel keep it, and Save closes it after writing", () => {
    expect(afterCloseChoice("discard")).toEqual({ close: true, save: false });
    expect(afterCloseChoice("save")).toEqual({ close: false, save: true });
    expect(afterCloseChoice("cancel")).toEqual({ close: false, save: false });
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
