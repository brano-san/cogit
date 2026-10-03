import { beforeEach, describe, expect, it, vi } from "vitest";

const handlers: { action?: (action: string, id: number | null) => void } = {};
const emitted = { queue: [] as unknown[][], reported: [] as unknown[] };

const commands = {
  commandOutcome: vi.fn(),
  openErrorsWindow: vi.fn(async () => ({ status: "ok", data: null })),
};
const events = {
  errorQueue: {
    emit: vi.fn(async (entries: unknown[]) => void emitted.queue.push(entries)),
    listen: vi.fn(),
  },
  errorReported: {
    emit: vi.fn(async (entry: unknown) => void emitted.reported.push(entry)),
    listen: vi.fn(async () => () => {}),
  },
  errorsAction: {
    emit: vi.fn(),
    listen: vi.fn(async (cb: (e: { payload: { action: string; id: number | null } }) => void) => {
      handlers.action = (action, id) => cb({ payload: { action, id } });
      return () => {};
    }),
  },
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events }));

const { errorWindow } = await import("./error-window.svelte");
const { notices } = await import("./notices.svelte");
const { CogitError } = await import("$lib/ipc");

const record = (id: number, over: object = {}) => ({
  id,
  repo: "C:/repos/cogit",
  command: "git push origin master",
  exitCode: 1,
  stdout: "",
  stderr: "error: failed to push some refs",
  durationMs: 12,
  operation: "Push",
  severity: "failure",
  summary: "error: failed to push some refs",
  startedAtMs: 1,
  stoppedOnConflicts: false,
  ...over,
});

/** The event carries no output and no command: the journal has them. */
const event = (id: number, over: object = {}) => {
  const { id: _id, repo, operation, summary, stoppedOnConflicts } = record(id, over);
  return { id: _id, repo, operation, summary, stoppedOnConflicts };
};

const stash = (id: number) =>
  record(id, {
    command: "git stash apply --index stash@{0}",
    operation: "Stash",
    summary: "CONFLICT (modify/delete): src/plate_navboard2.cpp",
    stoppedOnConflicts: true,
  });

const act = (action: string, id: number | null = null) => handlers.action?.(action, id);
let stop = () => {};

beforeEach(async () => {
  stop();
  commands.openErrorsWindow.mockClear();
  commands.commandOutcome.mockReset();
  commands.commandOutcome.mockImplementation((id: number) =>
    Promise.resolve(record(id, { summary: `failed ${id}` })),
  );
  stop = await errorWindow.own(() => {});
  act("closed");
  emitted.queue.length = 0;
  emitted.reported.length = 0;
});

describe("a failed command and the Errors window", () => {
  it("opens the window and puts the command there", async () => {
    await errorWindow.command(event(7));

    expect(errorWindow.entries).toHaveLength(1);
    expect(errorWindow.entries[0]).toMatchObject({ id: 7, title: "Push failed", kind: "error" });
    expect(commands.openErrorsWindow).toHaveBeenCalled();
    expect(emitted.queue.at(-1)).toHaveLength(1);
  });

  it("takes the operation's own repository and command", async () => {
    await errorWindow.command(event(7));

    expect(errorWindow.entries[0]).toMatchObject({
      repo: "C:/repos/cogit",
      command: "git push origin master",
    });
  });

  it("lists several in a row in the one list", async () => {
    await errorWindow.command(event(1, { summary: "a" }));
    await errorWindow.command(event(2, { summary: "b" }));
    await errorWindow.command(event(3, { summary: "c" }));

    expect(errorWindow.entries.map((entry) => entry.id)).toEqual([3, 2, 1]);
    expect(emitted.queue.at(-1)).toHaveLength(3);
  });

  it("counts the same failure again instead of listing it again", async () => {
    commands.commandOutcome.mockImplementation((id: number) => Promise.resolve(record(id)));

    await errorWindow.command(event(1));
    await errorWindow.command(event(2));

    expect(errorWindow.entries).toHaveLength(1);
    expect(errorWindow.entries[0]).toMatchObject({ id: 2, repeats: 2 });
  });

  it("queues the event and the rejected call as one", async () => {
    await Promise.all([errorWindow.command(event(7)), errorWindow.command(event(7))]);
    expect(errorWindow.entries).toHaveLength(1);
  });

  it("is reached from the notification store, and no toast is made", async () => {
    notices.dismissAll();
    const rejected = new CogitError({ kind: "command", data: record(9) as never });

    notices.report(rejected, "Could not push");
    await vi.waitFor(() => expect(errorWindow.entries).toHaveLength(1));

    expect(notices.all).toHaveLength(0);
    expect(errorWindow.entries[0]?.id).toBe(9);
  });

  it("is sent to the main window's queue by any other page", async () => {
    stop();

    await errorWindow.command(event(7));

    expect(errorWindow.entries).toHaveLength(0);
    expect(emitted.reported).toHaveLength(1);
  });
});

describe("a command that stops on conflicts", () => {
  it("is a warning, titled for what it did, and not a footer error", async () => {
    commands.commandOutcome.mockResolvedValue(stash(5));
    await errorWindow.command(event(5, { stoppedOnConflicts: true }));

    expect(errorWindow.entries[0]).toMatchObject({
      kind: "warning",
      title: "Stash applied with conflicts",
    });
    expect(errorWindow.warningPresent).toBe(true);
    expect(errorWindow.errorCount).toBe(0);
  });

  it("is listed after the errors", async () => {
    commands.commandOutcome.mockResolvedValueOnce(stash(5));
    await errorWindow.command(event(5, { stoppedOnConflicts: true }));
    await errorWindow.command(event(6));

    expect(errorWindow.entries.map((entry) => entry.kind)).toEqual(["error", "warning"]);
  });

  it("goes when the user opens the conflicts, and names the repository", async () => {
    const shown = vi.fn();
    stop();
    stop = await errorWindow.own(shown);
    commands.commandOutcome.mockResolvedValue(stash(5));
    await errorWindow.command(event(5, { stoppedOnConflicts: true }));

    act("showConflicts", 5);

    expect(shown).toHaveBeenCalledWith("C:/repos/cogit");
    expect(errorWindow.warningPresent).toBe(false);
  });

  it("goes when the conflicts are resolved, not before the status has had its say", async () => {
    vi.useFakeTimers();
    commands.commandOutcome.mockResolvedValue(stash(5));
    await errorWindow.command(event(5, { stoppedOnConflicts: true }));

    errorWindow.conflictsResolved("C:/repos/cogit");
    expect(errorWindow.warningPresent).toBe(true);

    vi.advanceTimersByTime(5_000);
    errorWindow.conflictsResolved("C:/repos/other");
    expect(errorWindow.warningPresent).toBe(true);
    errorWindow.conflictsResolved("C:/repos/cogit");
    expect(errorWindow.warningPresent).toBe(false);
    vi.useRealTimers();
  });
});

describe("a warning of a repository named with the other slash", () => {
  // The journal spells the root the way the OS does; the repository list normalizes it.
  it("goes with the conflicts all the same", async () => {
    vi.useFakeTimers();
    commands.commandOutcome.mockResolvedValue(stash(6));
    await errorWindow.command(event(6, { repo: "C:\\repos\\cogit", stoppedOnConflicts: true }));

    vi.advanceTimersByTime(5_000);
    errorWindow.conflictsResolved("C:/repos/cogit");

    expect(errorWindow.warningPresent).toBe(false);
    vi.useRealTimers();
  });
});

describe("the queue the window reads and the footer and taskbar watch", () => {
  it("counts what nobody has had on screen yet", async () => {
    await errorWindow.command(event(1, { summary: "a" }));
    await errorWindow.command(event(2, { summary: "b" }));
    expect(errorWindow.unviewed).toBe(2);
    expect(errorWindow.unviewedErrors).toBe(2);

    act("viewed", 2);

    expect(errorWindow.unviewed).toBe(1);
    expect(errorWindow.errorCount).toBe(2);
  });

  it("shows the next one when one is dismissed; the footer clears with the last", async () => {
    await errorWindow.command(event(1, { summary: "a" }));
    await errorWindow.command(event(2, { summary: "b" }));

    act("dismiss", 2);
    expect(errorWindow.entries.map((entry) => entry.id)).toEqual([1]);
    act("dismiss", 1);

    expect(errorWindow.errorCount).toBe(0);
    expect(emitted.queue.at(-1)).toEqual([]);
  });

  it("clears everything when the window is closed", async () => {
    await errorWindow.command(event(1, { summary: "a" }));
    await errorWindow.command(event(2, { summary: "b" }));

    act("closed");

    expect(errorWindow.entries).toEqual([]);
    expect(errorWindow.unviewed).toBe(0);
  });

  it("sends the queue to a window that has just started listening", async () => {
    await errorWindow.command(event(1));
    emitted.queue.length = 0;

    act("ready");

    expect(emitted.queue).toHaveLength(1);
  });
});
