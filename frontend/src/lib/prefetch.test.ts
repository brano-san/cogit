import { describe, expect, it, vi } from "vitest";
import { prefetcher } from "./prefetch";

describe("prefetcher", () => {
  it("serves a prefetched answer synchronously, loading once", async () => {
    const cache = prefetcher<number>();
    const scope = {};
    const load = vi.fn(() => Promise.resolve(7));
    expect(cache.peek(scope, "a")).toBeUndefined();
    await cache.get(scope, "a", load);
    expect(cache.peek(scope, "a")).toBe(7);
    await cache.get(scope, "a", load);
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("forgets answers of an old scope and failed loads", async () => {
    const cache = prefetcher<number>();
    const scope = {};
    await cache.get(scope, "a", () => Promise.resolve(1));
    expect(cache.peek({}, "a")).toBeUndefined();
    await cache.get(scope, "b", () => Promise.reject(new Error("x"))).catch(() => {});
    const again = vi.fn(() => Promise.resolve(2));
    await cache.get(scope, "b", again);
    expect(again).toHaveBeenCalledOnce();
  });
});
