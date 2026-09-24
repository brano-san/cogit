import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

const readPulse = vi.fn(async () => ({ missing: false }));
vi.mock("$lib/ipc/repo-rows", () => ({ repoPulse: readPulse, pullProbe: vi.fn(async () => false) }));

describe("the row a repository leaves on screen", () => {
  beforeAll(() => import("./repo-pulse.svelte"), 60_000);
  afterEach(() => vi.useRealTimers());

  // Reading it inside the close was the `repo.close` slowdown of 24.09.
  it("is read once the close has settled, not during it", async () => {
    vi.useFakeTimers();
    vi.resetModules();
    const { repoPulse } = await import("./repo-pulse.svelte");
    repoPulse.setOwned("C:/repos/one");
    repoPulse.watch(["C:/repos/one"]);
    await vi.advanceTimersByTimeAsync(2_000);
    readPulse.mockClear();

    repoPulse.setOwned(null);
    await vi.advanceTimersByTimeAsync(100);
    expect(readPulse).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1_000);
    expect(readPulse).toHaveBeenCalledWith("C:/repos/one");
  });
});
