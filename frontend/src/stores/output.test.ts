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
  });

  it("opens the window on a failure, with the output the notice did not carry", async () => {
    await output.notice(notice(7, "failure"));

    expect(output.shown?.id).toBe(7);
    expect(output.shown?.stderr).toContain("failed to push");
  });

  it("does not open a second window for a second failure", async () => {
    await output.notice(notice(7, "failure"));
    await output.notice(notice(8, "failure"));

    expect(output.shown?.id).toBe(7);
    expect(output.unread).toBe(1);
  });

  it("moves to the newest failure when asked, and stops counting", async () => {
    await output.notice(notice(7, "failure"));
    await output.notice(notice(8, "failure"));
    await output.notice(notice(9, "failure"));

    await output.showNewest();

    expect(output.shown?.id).toBe(9);
    expect(output.unread).toBe(0);
  });

  it("leaves the window alone for a warning, which is what makes commits bearable", async () => {
    await output.notice(notice(3, "warning"));

    expect(output.shown).toBeNull();
    expect(output.warning?.summary).toBe("summary 3");
  });

  it("says nothing at all about a command that worked", async () => {
    await output.notice(notice(4, "success"));

    expect(output.shown).toBeNull();
    expect(output.warning).toBeNull();
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

  it("forgets the unread count when the window is closed", async () => {
    await output.notice(notice(7, "failure"));
    await output.notice(notice(8, "failure"));

    output.close();

    expect(output.unread).toBe(0);
    expect(output.shown).toBeNull();
  });

  it("survives a record that has already rotated out of the journal", async () => {
    commands.commandOutcome.mockResolvedValue(null);

    await output.notice(notice(7, "failure"));

    expect(output.shown).toBeNull();
  });
});

describe("output store, when a failure arrives twice", () => {
  beforeEach(() => {
    commands.commandOutcome.mockReset();
    commands.commandOutcome.mockImplementation((id: number) => Promise.resolve(record(id)));
    output.close();
  });

  it("counts the event and the rejected call as one failure", async () => {
    await Promise.all([output.notice(notice(7, "failure")), output.raise(7)]);

    expect(output.shown?.id).toBe(7);
    expect(output.unread).toBe(0);
    expect(commands.commandOutcome).toHaveBeenCalledTimes(1);
  });

  it("does not count an unread failure twice either", async () => {
    await output.notice(notice(7, "failure"));
    await output.notice(notice(8, "failure"));
    await output.raise(8);

    expect(output.unread).toBe(1);
  });
});

describe("the queue behind the output window", () => {
  beforeEach(() => {
    commands.commandOutcome.mockReset();
    commands.commandOutcome.mockImplementation((id: number) => Promise.resolve(record(id)));
    output.close();
  });

  it("keeps every failure, not only the one on screen", async () => {
    for (const id of [1, 2, 3]) await output.notice(notice(id, "failure"));

    expect(output.queue).toEqual([1, 2, 3]);
    expect(output.at).toBe(0);
  });

  it("walks forward and back through them", async () => {
    for (const id of [1, 2, 3]) await output.notice(notice(id, "failure"));

    await output.step(1);
    expect(output.shown?.id).toBe(2);
    await output.step(1);
    expect(output.shown?.id).toBe(3);
    await output.step(-1);
    expect(output.shown?.id).toBe(2);
  });

  it("does not walk off either end", async () => {
    for (const id of [1, 2]) await output.notice(notice(id, "failure"));

    await output.step(-1);
    expect(output.shown?.id).toBe(1);
    await output.step(1);
    await output.step(1);
    expect(output.shown?.id).toBe(2);
  });

  // `Close` used to throw the other two away.
  it("closing one moves to the next and only the last closes the window", async () => {
    for (const id of [1, 2]) await output.notice(notice(id, "failure"));

    await output.dismissShown();
    expect(output.shown?.id).toBe(2);
    await output.dismissShown();
    expect(output.shown).toBeNull();
  });

  // Fifteen identical pull failures are one thing that happened fifteen times.
  it("counts a repeat instead of queueing it again", async () => {
    await output.notice(notice(1, "failure"));
    await output.notice({ ...notice(2, "failure"), summary: "summary 1" });

    expect(output.queue).toHaveLength(1);
    expect(output.repeats).toBe(2);
  });

  it("starts counting again once something different fails", async () => {
    await output.notice(notice(1, "failure"));
    await output.notice({ ...notice(2, "failure"), summary: "summary 1" });
    await output.notice({ ...notice(3, "failure"), summary: "another thing" });

    expect(output.queue).toHaveLength(2);
    expect(output.repeats).toBe(1);
  });
});
