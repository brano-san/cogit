import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { successToast, TOAST_FADE_MS, TOAST_HOLD_MS } from "./success-toast.svelte";

describe("successToast", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    successToast.clear();
    vi.useRealTimers();
  });

  it("shows, fades, then goes away", () => {
    successToast.show("Pull succeeded");
    expect(successToast.text).toBe("Pull succeeded");
    vi.advanceTimersByTime(TOAST_HOLD_MS);
    expect(successToast.leaving).toBe(true);
    vi.advanceTimersByTime(TOAST_FADE_MS);
    expect(successToast.text).toBeNull();
    expect(successToast.leaving).toBe(false);
  });

  it("a newer toast replaces the old one and restarts the clock", () => {
    successToast.show("Fetch succeeded");
    vi.advanceTimersByTime(TOAST_HOLD_MS + 100);
    const first = successToast.id;
    successToast.show("Push succeeded");
    expect(successToast.text).toBe("Push succeeded");
    expect(successToast.leaving).toBe(false);
    expect(successToast.id).toBe(first + 1);
    vi.advanceTimersByTime(TOAST_HOLD_MS - 1);
    expect(successToast.text).toBe("Push succeeded");
    vi.advanceTimersByTime(TOAST_FADE_MS + 1);
    expect(successToast.text).toBeNull();
  });
});
