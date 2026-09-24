import { describe as group, expect, it, vi } from "vitest";
import { checkForUpdates, describe, message, type UpdateHandle, type Updates } from "./updates";

function handle(over: Partial<UpdateHandle> = {}): UpdateHandle {
  return {
    version: "0.2.0",
    body: "Faster graph.",
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    ...over,
  };
}

function io(over: Partial<Updates> = {}): Updates {
  return {
    check: vi.fn().mockResolvedValue(null),
    relaunch: vi.fn().mockResolvedValue(undefined),
    confirm: vi.fn().mockReturnValue(true),
    report: vi.fn(),
    ...over,
  };
}

group("describe", () => {
  it("calls nothing found being up to date", () => {
    expect(describe(null)).toEqual({ kind: "none" });
  });

  it("carries the version and trims the notes", () => {
    expect(describe(handle({ body: "  notes  " }))).toEqual({
      kind: "available",
      version: "0.2.0",
      notes: "notes",
    });
  });

  it("copes with a release that has no notes", () => {
    expect(describe(handle({ body: null }))).toEqual({
      kind: "available",
      version: "0.2.0",
      notes: "",
    });
  });
});

group("message", () => {
  it("leaves out the empty notes rather than showing a blank gap", () => {
    expect(message({ kind: "available", version: "0.2.0", notes: "" })).toBe(
      "Cogit 0.2.0 is available. Install it and restart?",
    );
  });

  it("shows the notes when there are some", () => {
    expect(message({ kind: "available", version: "0.2.0", notes: "Faster." })).toContain("Faster.");
  });

  it("says why a check failed", () => {
    expect(message({ kind: "failed", reason: "offline" })).toContain("offline");
  });
});

group("checkForUpdates", () => {
  it("reports being up to date and installs nothing", async () => {
    const side = io();

    const outcome = await checkForUpdates(side);

    expect(outcome).toEqual({ kind: "none" });
    expect(side.report).toHaveBeenCalledWith("Cogit is up to date.");
    expect(side.relaunch).not.toHaveBeenCalled();
  });

  it("installs and relaunches once the user agrees", async () => {
    const found = handle();
    const side = io({ check: vi.fn().mockResolvedValue(found) });

    await checkForUpdates(side);

    expect(found.downloadAndInstall).toHaveBeenCalledOnce();
    expect(side.relaunch).toHaveBeenCalledOnce();
  });

  it("installs nothing when the user declines", async () => {
    const found = handle();
    const side = io({ check: vi.fn().mockResolvedValue(found), confirm: vi.fn(() => false) });

    await checkForUpdates(side);

    expect(found.downloadAndInstall).not.toHaveBeenCalled();
    expect(side.relaunch).not.toHaveBeenCalled();
  });

  // The question has to be the app's own dialog, which answers later; a pending answer
  // is a truthy promise, and the update installed before the user said no.
  it("waits for a no that comes later", async () => {
    const found = handle();
    const side = io({ check: vi.fn().mockResolvedValue(found), confirm: vi.fn(async () => false) });

    await checkForUpdates(side);

    expect(found.downloadAndInstall).not.toHaveBeenCalled();
  });

  it("a failed check is told to the user, not swallowed", async () => {
    const side = io({ check: vi.fn().mockRejectedValue(new Error("no network")) });

    const outcome = await checkForUpdates(side);

    expect(outcome.kind).toBe("failed");
    expect(vi.mocked(side.report).mock.calls[0]?.[0]).toContain("no network");
  });

  it("a failed install does not relaunch into a half-written app", async () => {
    const found = handle({ downloadAndInstall: vi.fn().mockRejectedValue(new Error("disk full")) });
    const side = io({ check: vi.fn().mockResolvedValue(found) });

    const outcome = await checkForUpdates(side);

    expect(outcome.kind).toBe("failed");
    expect(side.relaunch).not.toHaveBeenCalled();
    expect(vi.mocked(side.report).mock.calls[0]?.[0]).toContain("disk full");
  });
});

group("the start-up check", () => {
  it("says nothing when there is nothing to install", async () => {
    const side = io();

    const outcome = await checkForUpdates(side, { quiet: true });

    expect(outcome).toEqual({ kind: "none" });
    expect(side.report).not.toHaveBeenCalled();
  });

  it("says nothing when the network is down", async () => {
    const side = io({ check: vi.fn().mockRejectedValue(new Error("offline")) });

    const outcome = await checkForUpdates(side, { quiet: true });

    expect(outcome.kind).toBe("failed");
    expect(side.report).not.toHaveBeenCalled();
  });

  it("still asks before installing anything", async () => {
    const found = handle();
    const side = io({ check: vi.fn().mockResolvedValue(found), confirm: vi.fn(() => false) });

    await checkForUpdates(side, { quiet: true });

    expect(side.confirm).toHaveBeenCalledOnce();
    expect(found.downloadAndInstall).not.toHaveBeenCalled();
  });

  it("a failed install is still reported, quiet or not", async () => {
    const found = handle({ downloadAndInstall: vi.fn().mockRejectedValue(new Error("disk full")) });
    const side = io({ check: vi.fn().mockResolvedValue(found) });

    await checkForUpdates(side, { quiet: true });

    expect(vi.mocked(side.report).mock.calls[0]?.[0]).toContain("disk full");
  });
});
