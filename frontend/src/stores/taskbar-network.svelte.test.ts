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

const { network } = await import("./network.svelte");

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

// A failed pull ended "well" for the button as well as badly: two unviewed events for one.
describe("a failed network operation", () => {
  it("is still one success event when it went well", async () => {
    const stop = taskbar.start();
    taskbar.viewed();
    flushSync();

    const done = network.run(null, "Pulling", async () => {
      await Promise.resolve();
    });
    flushSync();
    await done;
    flushSync();

    expect(taskbar.unviewed).toBe(1);
    stop();
  });

  it("is one unviewed event, the error, and not a success as well", async () => {
    const stop = taskbar.start();
    taskbar.viewed();
    flushSync();

    const pull = network.run(null, "Pulling", async () => {
      await Promise.resolve();
      throw new Error("refused");
    });
    flushSync();
    await expect(pull).rejects.toThrow("refused");
    errorWindow.add(entry(10, "error") as never);
    flushSync();

    expect(taskbar.unviewed).toBe(1);
    stop();
  });
});
