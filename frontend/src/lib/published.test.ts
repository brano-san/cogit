import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PUBLISHED_WAIT_MS, publishedOrAssume } from "./published";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("publishedOrAssume", () => {
  it("passes a timely answer through", async () => {
    await expect(publishedOrAssume(Promise.resolve(false))).resolves.toBe(false);
    await expect(publishedOrAssume(Promise.resolve(true))).resolves.toBe(true);
  });

  it("counts a failed check as published", async () => {
    await expect(publishedOrAssume(Promise.reject(new Error("git failed")))).resolves.toBe(true);
  });

  it("counts a check that has not answered in time as published", async () => {
    const never = new Promise<boolean>(() => {});
    const answer = publishedOrAssume(never);
    await vi.advanceTimersByTimeAsync(PUBLISHED_WAIT_MS);
    await expect(answer).resolves.toBe(true);
  });

  it("ignores an answer that arrives after the wait", async () => {
    let settle: (value: boolean) => void = () => {};
    const late = new Promise<boolean>((resolve) => (settle = resolve));
    const answer = publishedOrAssume(late, 50);
    await vi.advanceTimersByTimeAsync(50);
    settle(false);
    await expect(answer).resolves.toBe(true);
  });
});
