import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createGitChecker, describeCheck, type GitCheck, type Probe } from "./git-check";

const ok = (version: string): Probe => ({ valid: true, version, error: null, olderThan: null });

describe("createGitChecker", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("waits for typing to pause and probes once with the last value", async () => {
    const probe = vi.fn(async (path: string) => ok(path));
    const seen: GitCheck[] = [];
    const checker = createGitChecker(probe, (c) => seen.push(c), 300);
    checker.check("g");
    checker.check("gi");
    checker.check("git");
    await vi.advanceTimersByTimeAsync(300);
    expect(probe).toHaveBeenCalledTimes(1);
    expect(probe).toHaveBeenCalledWith("git");
    expect(seen.at(-1)).toEqual({ state: "ok", version: "git" });
  });

  it("drops an answer that a newer check has overtaken", async () => {
    let release: (p: Probe) => void = () => {};
    const probe = vi
      .fn<(path: string) => Promise<Probe>>()
      .mockImplementationOnce(() => new Promise((resolve) => (release = resolve)))
      .mockResolvedValueOnce(ok("2.50.0"));
    const seen: GitCheck[] = [];
    const checker = createGitChecker(probe, (c) => seen.push(c), 10);
    checker.check("old");
    await vi.advanceTimersByTimeAsync(10);
    checker.check("new");
    await vi.advanceTimersByTimeAsync(10);
    release(ok("1.0.0"));
    await vi.advanceTimersByTimeAsync(0);
    expect(seen.at(-1)).toEqual({ state: "ok", version: "2.50.0" });
    expect(seen).not.toContainEqual({ state: "ok", version: "1.0.0" });
  });

  it("reports a failed probe and a rejected call as bad", async () => {
    const probe = vi
      .fn<(path: string) => Promise<Probe>>()
      .mockResolvedValueOnce({ valid: false, version: null, error: "cannot run x", olderThan: null })
      .mockRejectedValueOnce(new Error("ipc down"));
    const seen: GitCheck[] = [];
    const checker = createGitChecker(probe, (c) => seen.push(c), 1);
    checker.check("x");
    await vi.advanceTimersByTimeAsync(1);
    expect(seen.at(-1)).toEqual({ state: "bad", reason: "cannot run x" });
    checker.check("y");
    await vi.advanceTimersByTimeAsync(1);
    expect(seen.at(-1)).toEqual({ state: "bad", reason: "ipc down" });
  });

  it("stops after dispose", async () => {
    const probe = vi.fn(async () => ok("1"));
    const seen: GitCheck[] = [];
    const checker = createGitChecker(probe, (c) => seen.push(c), 5);
    checker.check("a");
    checker.dispose();
    await vi.advanceTimersByTimeAsync(50);
    expect(probe).not.toHaveBeenCalled();
    expect(seen.at(-1)?.state).toBe("checking");
  });
});

describe("describeCheck", () => {
  it("words each state", () => {
    expect(describeCheck({ state: "ok", version: "2.41.0" })).toBe("Version: 2.41.0");
    expect(describeCheck({ state: "bad", reason: "nope" })).toBe("nope");
    expect(describeCheck({ state: "checking" })).toBe("Checking…");
    expect(describeCheck({ state: "idle" })).toBe("");
  });
});
