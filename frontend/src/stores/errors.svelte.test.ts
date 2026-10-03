import { afterEach, describe, expect, it, vi } from "vitest";
import { flushSync } from "svelte";

const commands = { commandOutcome: vi.fn(), repositoryHealth: vi.fn(), readSettings: vi.fn(), writeSetting: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { CogitError } = await import("$lib/ipc");
const { notices } = await import("./notices.svelte");
const { errors } = await import("./errors.svelte");

describe("errors.report called from an effect", () => {
  afterEach(() => vi.useRealTimers());

  // The effect in App.svelte reports `diff.error`; reading the queue to dedupe made it
  // depend on the queue it writes to, so a dismissed error came straight back.
  it("does not bring a dismissed error back when the queue changes", () => {
    vi.useFakeTimers();
    notices.dismissAll();
    const failure = $state.raw(new CogitError({ kind: "invalidState", data: "boom" }));
    const stop = $effect.root(() => {
      $effect(() => errors.report(failure, "Could not show the diff", "a.md"));
    });
    flushSync();
    expect(notices.all).toHaveLength(1);

    vi.advanceTimersByTime(15_000);
    notices.dismiss();
    flushSync();
    expect(notices.all).toHaveLength(0);

    notices.inform("x", "y");
    flushSync();
    expect(notices.all.filter((notice) => notice.severity === "error")).toHaveLength(0);
    stop();
  });
});
