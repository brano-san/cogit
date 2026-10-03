import { describe, expect, it, vi } from "vitest";
import { flushSync } from "svelte";

const commands = {
  setTaskbarState: vi.fn(async () => {}),
  openErrorsWindow: vi.fn(async () => {}),
  publishErrorQueue: vi.fn(async () => {}),
  readSettings: vi.fn(),
  writeSetting: vi.fn(),
  commandOutcome: vi.fn(),
  repositoryHealth: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));
vi.stubGlobal("document", { hasFocus: () => false });
vi.stubGlobal("window", { addEventListener() {}, removeEventListener() {} });

const { taskbar } = await import("./taskbar.svelte");
const { errorWindow } = await import("./error-window.svelte");

const entry = (id: number, kind: "error" | "warning") => ({
  id,
  kind,
  title: "t",
  operation: "Pull",
  repo: "C:/r",
  command: "git pull",
  summary: "s",
  repeats: 1,
});

const lastSignals = () => {
  const calls = commands.setTaskbarState.mock.calls as unknown as [Record<string, unknown>][];
  return calls[calls.length - 1]?.[0];
};

// A failed fetch/pull/push goes to the Errors window, not to the notification queue the
// taskbar read, so the button showed it as a success.
describe("taskbar and the Errors window", () => {
  it("counts a failed command as an error and is an unviewed event", () => {
    const stop = taskbar.start();
    flushSync();
    errorWindow.add(entry(1, "error") as never);
    flushSync();
    expect(lastSignals()).toMatchObject({ errors: 1 });
    expect(taskbar.unviewed).toBe(1);
    stop();
  });

  it("counts a conflict warning too", () => {
    const stop = taskbar.start();
    flushSync();
    errorWindow.add(entry(2, "warning") as never);
    flushSync();
    expect(lastSignals()).toMatchObject({ warnings: 1 });
    stop();
  });
});
