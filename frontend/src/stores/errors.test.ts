import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const commands = { commandOutcome: vi.fn(), repositoryHealth: vi.fn(), readSettings: vi.fn(), writeSetting: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { CogitError } = await import("$lib/ipc");
const { notices } = await import("./notices.svelte");
const { errors } = await import("./errors.svelte");

const refusal = (text: string) => new CogitError({ kind: "invalidState", data: text });

describe("errors.report with a source", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    notices.dismissAll();
    // Past any quiet period a previous test left behind.
    vi.advanceTimersByTime(60_000);
  });
  afterEach(() => vi.useRealTimers());

  it("shows the same failure of the same file once while it is pending", () => {
    for (let refresh = 0; refresh < 5; refresh += 1) {
      errors.report(refusal("boom"), "Could not show the diff", "a.md");
    }
    expect(notices.all).toHaveLength(1);
    expect(notices.all[0]?.repeats).toBe(1);
  });

  it("stays quiet for a while after it was dismissed, then speaks again", () => {
    errors.report(refusal("boom"), "Could not show the diff", "a.md");
    notices.dismiss();

    vi.advanceTimersByTime(5_000);
    errors.report(refusal("boom"), "Could not show the diff", "a.md");
    expect(notices.all).toHaveLength(0);

    vi.advanceTimersByTime(11_000);
    errors.report(refusal("boom"), "Could not show the diff", "a.md");
    expect(notices.all).toHaveLength(1);
  });

  it("does not mix up operations or messages", () => {
    errors.report(refusal("boom"), "Could not show the diff", "c.md");
    errors.report(refusal("boom"), "Could not discard the lines", "c.md");
    errors.report(refusal("other"), "Could not show the diff", "c.md");
    expect(notices.all.map((notice) => notice.title)).toEqual([
      "Could not show the diff",
      "Could not discard the lines",
      "Could not show the diff",
    ]);
  });

  it("without a source behaves as before: nothing is dropped", () => {
    errors.report(refusal("boom"), "Could not merge");
    errors.report(refusal("boom"), "Could not merge");
    expect(notices.all).toHaveLength(1);
    notices.dismiss();
    errors.report(refusal("boom"), "Could not merge");
    expect(notices.all).toHaveLength(1);
  });
});
