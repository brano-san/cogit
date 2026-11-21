import { describe, expect, it, vi } from "vitest";
import { probeRoots, retrySshHost, sshHost } from "./ssh-repositories";

describe("SSH repositories", () => {
  it("uses the backend label as the source of the SSH destination", () => {
    expect(sshHost("SSH: me@[::1]:2222")).toBe("me@[::1]:2222");
    expect(sshHost("WSL: Ubuntu")).toBeNull();
    expect(sshHost(undefined)).toBeNull();
  });

  it("never probes restored SSH roots or roots awaiting classification", () => {
    const labels = new Map([["a", "local"], ["b", "SSH: server"], ["c", "SSH: build"]]);
    expect(probeRoots(["a", "b", "c", "d"], labels, ["c"], new Set())).toEqual(["a", "c"]);
    expect(probeRoots(["a", "b", "c", "d"], labels, ["c"], new Set(["SSH: build"]))).toEqual(["a"]);
  });

  it("backs off between attempts and stops after recovery", async () => {
    const waits: number[] = [];
    const attempt = vi.fn().mockRejectedValueOnce(Error("down")).mockRejectedValueOnce(Error("down")).mockResolvedValue(undefined);
    await retrySshHost(attempt, async (ms) => { waits.push(ms); });
    expect(waits).toEqual([1000, 2000, 4000]);
    expect(attempt).toHaveBeenCalledTimes(3);
  });

  it("bounds automatic retries and preserves the last error", async () => {
    const waits: number[] = [];
    const attempt = vi.fn().mockRejectedValue(Error("connection lost"));
    await expect(retrySshHost(attempt, async (ms) => { waits.push(ms); })).rejects.toThrow("connection lost");
    expect(waits).toEqual([1000, 2000, 4000, 8000, 16000]);
  });
});
