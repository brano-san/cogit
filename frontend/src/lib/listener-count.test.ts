import { beforeEach, describe, expect, it, vi } from "vitest";
import { counted, liveListeners, resetListenerCount } from "$lib/listener-count";

beforeEach(resetListenerCount);

describe("counted", () => {
  it("counts a subscription up and back down", async () => {
    const stop = await counted(Promise.resolve(() => {}));
    expect(liveListeners()).toBe(1);
    stop();
    expect(liveListeners()).toBe(0);
  });

  it("passes the stop through to the real listener", async () => {
    const real = vi.fn();
    const stop = await counted(Promise.resolve(real));
    stop();
    expect(real).toHaveBeenCalledTimes(1);
  });

  it("ignores a second stop, so a leak cannot hide behind a negative count", async () => {
    const real = vi.fn();
    const stop = await counted(Promise.resolve(real));
    stop();
    stop();
    expect(liveListeners()).toBe(0);
    expect(real).toHaveBeenCalledTimes(1);
  });

  it("shows subscriptions that were never stopped", async () => {
    await counted(Promise.resolve(() => {}));
    await counted(Promise.resolve(() => {}));
    const stop = await counted(Promise.resolve(() => {}));
    stop();
    expect(liveListeners()).toBe(2);
  });

  it("counts nothing when the subscription never attached", async () => {
    await expect(counted(Promise.reject(new Error("no backend")))).rejects.toThrow();
    expect(liveListeners()).toBe(0);
  });
});
