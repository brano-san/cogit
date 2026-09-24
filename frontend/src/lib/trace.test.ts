import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = { logFromFrontend: vi.fn() };
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { format, line, startTracing, timed, trace } = await import("./trace");

describe("trace lines", () => {
  beforeEach(() => {
    commands.logFromFrontend.mockReset();
    commands.logFromFrontend.mockResolvedValue(undefined);
    startTracing(1000);
  });

  it("counts from the moment tracing started, not from the epoch", () => {
    expect(line("open", "asked the backend", 1250).at).toBe(250);
  });

  it("puts the elapsed time where a reader looking for a gap will see it", () => {
    expect(format(line("open", "asked the backend", 1250))).toBe("+250ms asked the backend");
  });

  it("keeps the context apart from the message, so one story can be grepped out", () => {
    trace("open:C:/repos/one", "phase → opening");
    expect(commands.logFromFrontend).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("phase → opening"),
      "open:C:/repos/one",
    );
  });

  // Tracing that can break what it traces is worse than no tracing.
  it("survives a backend that refuses the call", async () => {
    commands.logFromFrontend.mockRejectedValue(new Error("no"));
    expect(() => trace("open", "still fine")).not.toThrow();
  });

  it("survives there being no bridge at all", () => {
    commands.logFromFrontend.mockImplementation(() => {
      throw new TypeError("logFromFrontend is not a function");
    });
    expect(() => trace("open", "still fine")).not.toThrow();
  });

  it("times a step that works and returns its value", async () => {
    await expect(timed("open", "read branches", () => Promise.resolve(9))).resolves.toBe(9);
    expect(commands.logFromFrontend.mock.calls[0]?.[1]).toContain("read branches ok");
  });

  it("times a step that fails, says so, and still throws", async () => {
    await expect(
      timed("open", "read branches", () => Promise.reject(new Error("locked"))),
    ).rejects.toThrow("locked");
    expect(commands.logFromFrontend.mock.calls[0]?.[1]).toContain("read branches failed");
    expect(commands.logFromFrontend.mock.calls[0]?.[1]).toContain("locked");
  });
});
