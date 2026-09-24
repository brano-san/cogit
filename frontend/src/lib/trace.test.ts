import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const commands = { logFromFrontend: vi.fn() };
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { TRACE_BATCH_LINES, TRACE_BATCH_MS, flushTrace, format, line, startTracing, timed, trace } =
  await import("./trace");

type Sent = { level: string; message: string; context: string };
const batches = (): Sent[][] => commands.logFromFrontend.mock.calls.map((call) => call[0] as Sent[]);
const sent = (): Sent[] => batches().flat();

describe("trace lines", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    commands.logFromFrontend.mockReset();
    commands.logFromFrontend.mockResolvedValue(undefined);
    startTracing(1000);
  });
  afterEach(() => {
    flushTrace();
    vi.useRealTimers();
  });

  it("counts from the moment tracing started, not from the epoch", () => {
    expect(line("open", "asked the backend", 1250).at).toBe(250);
  });

  it("puts the elapsed time where a reader looking for a gap will see it", () => {
    expect(format(line("open", "asked the backend", 1250))).toBe("+250ms asked the backend");
  });

  it("keeps the context apart from the message, so one story can be grepped out", () => {
    trace("open:C:/repos/one", "phase → opening");
    flushTrace();
    expect(sent()).toEqual([
      { level: "info", message: expect.stringContaining("phase → opening"), context: "open:C:/repos/one" },
    ]);
  });

  // Tracing that can break what it traces is worse than no tracing.
  it("survives a backend that refuses the call", async () => {
    commands.logFromFrontend.mockRejectedValue(new Error("no"));
    trace("open", "still fine");
    expect(() => flushTrace()).not.toThrow();
  });

  it("survives there being no bridge at all", () => {
    commands.logFromFrontend.mockImplementation(() => {
      throw new TypeError("logFromFrontend is not a function");
    });
    trace("open", "still fine");
    expect(() => flushTrace()).not.toThrow();
  });

  it("times a step that works and returns its value", async () => {
    await expect(timed("open", "read branches", () => Promise.resolve(9))).resolves.toBe(9);
    flushTrace();
    expect(sent()[0]?.message).toContain("read branches ok");
  });

  it("times a step that fails, says so, and still throws", async () => {
    await expect(
      timed("open", "read branches", () => Promise.reject(new Error("locked"))),
    ).rejects.toThrow("locked");
    expect(sent()[0]?.message).toContain("read branches failed");
    expect(sent()[0]?.message).toContain("locked");
    expect(sent()[0]?.level).toBe("error");
  });
});

describe("trace batching", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    commands.logFromFrontend.mockReset();
    commands.logFromFrontend.mockResolvedValue(undefined);
    startTracing(0);
  });
  afterEach(() => {
    flushTrace();
    vi.useRealTimers();
  });

  // Opening a repository wrote 9–12 lines, each its own IPC call.
  it("sends the lines of one action in a single call once the timer fires", async () => {
    for (let step = 0; step < 12; step += 1) trace("open:C:/repos/one", `step ${step}`);
    expect(commands.logFromFrontend).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(TRACE_BATCH_MS);

    expect(commands.logFromFrontend).toHaveBeenCalledTimes(1);
    expect(sent().map((entry) => entry.message.replace(/^\+\d+ms /, ""))).toEqual(
      Array.from({ length: 12 }, (_, step) => `step ${step}`),
    );
  });

  it("sends a full batch without waiting for the timer", () => {
    for (let step = 0; step < TRACE_BATCH_LINES; step += 1) trace("scroll", `line ${step}`);
    expect(commands.logFromFrontend).toHaveBeenCalledTimes(1);
    expect(batches()[0]).toHaveLength(TRACE_BATCH_LINES);
  });

  it("sends an error at once, after the lines that led to it", async () => {
    trace("open", "asked the backend");
    await expect(timed("open", "open repository", () => Promise.reject(new Error("locked")))).rejects.toThrow();

    expect(commands.logFromFrontend).toHaveBeenCalledTimes(1);
    expect(sent().map((entry) => entry.level)).toEqual(["info", "error"]);
  });

  it("sends what is waiting when the page goes away", () => {
    const page = new EventTarget();
    startTracing(0, page);
    trace("exit", "closing");
    page.dispatchEvent(new Event("pagehide"));
    expect(commands.logFromFrontend).toHaveBeenCalledTimes(1);
  });

  it("sends nothing when nothing waits", async () => {
    flushTrace();
    await vi.advanceTimersByTimeAsync(TRACE_BATCH_MS * 4);
    expect(commands.logFromFrontend).not.toHaveBeenCalled();
  });
});
