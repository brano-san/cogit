import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = {
  commandLog: vi.fn(),
  commandOutcome: vi.fn(),
  commandProblems: vi.fn(),
  clearCommandLog: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { output } = await import("./output.svelte");
const { notices } = await import("./notices.svelte");

type Severity = "success" | "warning" | "failure";

const notice = (id: number, severity: Severity) => ({
  id,
  repo: "C:/repos/cogit",
  operation: "Push",
  severity,
  summary: `summary ${id}`,
});

const record = (id: number) => ({
  id,
  repo: "C:/repos/cogit",
  command: "git push origin master",
  exitCode: 1,
  stdout: "",
  stderr: "error: failed to push some refs",
  durationMs: 12,
  operation: "Push",
  severity: "failure" as const,
  summary: "error: failed to push some refs",
  startedAtMs: 1,
});

describe("output store", () => {
  beforeEach(() => {
    commands.commandOutcome.mockReset();
    commands.commandOutcome.mockImplementation((id: number) => Promise.resolve(record(id)));
    output.close();
    output.dismissWarning();
    notices.dismissAll();
  });

  // One window for everything that went wrong: the output window opens on request.
  it("announces a failure in the notification window, not by opening itself", async () => {
    await output.notice(notice(7, "failure"));

    expect(output.shown).toBeNull();
    expect(notices.current?.title).toBe("Push failed");
    expect(notices.current?.record).toBe(7);
  });

  it("opens the record Show Output asks for, with the output the notice did not carry", async () => {
    await output.openRecord(7);

    expect(output.shown?.id).toBe(7);
    expect(output.shown?.stderr).toContain("failed to push");
  });

  it("stays closed for a record that has already rotated out of the journal", async () => {
    commands.commandOutcome.mockResolvedValue(null);

    await output.openRecord(7);

    expect(output.shown).toBeNull();
  });

  it("leaves the windows alone for a warning, which is what makes commits bearable", async () => {
    await output.notice(notice(3, "warning"));

    expect(output.shown).toBeNull();
    expect(notices.current).toBeUndefined();
    expect(output.warning?.summary).toBe("summary 3");
  });

  it("says nothing at all about a command that worked", async () => {
    await output.notice(notice(4, "success"));

    expect(output.shown).toBeNull();
    expect(output.warning).toBeNull();
    expect(notices.current).toBeUndefined();
  });

  it("opens the window from a warning when the reader asks for details", async () => {
    await output.notice(notice(3, "warning"));

    await output.showWarning();

    expect(output.shown?.id).toBe(3);
    expect(output.warning).toBeNull();
  });

  it("counts problems as they happen, without asking the backend again", async () => {
    const before = output.problems;

    await output.notice(notice(1, "failure"));
    await output.notice(notice(2, "warning"));
    await output.notice(notice(3, "success"));

    expect(output.problems).toBe(before + 2);
    expect(commands.commandProblems).not.toHaveBeenCalled();
  });
});
