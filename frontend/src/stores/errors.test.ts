import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = { commandOutcome: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { CogitError } = await import("$lib/ipc");
const { errors } = await import("./errors.svelte");

const refusal = (text: string) => new CogitError({ kind: "invalidState", data: text });

describe("errors store", () => {
  beforeEach(() => {
    errors.dismissAll();
  });

  it("has nothing to say until something goes wrong", () => {
    expect(errors.current).toBeNull();
    expect(errors.pending).toBe(0);
  });

  it("shows what went wrong", () => {
    errors.report(refusal("Not a Git repository"));

    expect(errors.current?.message).toContain("Not a Git repository");
    expect(errors.pending).toBe(1);
  });

  // The old store re-reported the same error on every effect run, which on a failed open
  // meant one notification per panel that read the store.
  it("does not repeat itself when the same failure is reported again", () => {
    errors.report(refusal("Not a Git repository"));
    errors.report(refusal("Not a Git repository"));

    expect(errors.pending).toBe(1);
  });

  it("queues a different failure behind the first, newest in front", () => {
    errors.report(refusal("first"));
    errors.report(refusal("second"));

    expect(errors.current?.message).toContain("second");
    expect(errors.pending).toBe(2);
  });

  it("moves to the one behind when the front one is dismissed", () => {
    errors.report(refusal("first"));
    errors.report(refusal("second"));

    errors.dismiss();

    expect(errors.current?.message).toContain("first");
    expect(errors.pending).toBe(1);
  });

  // Item 4: the footer follows the notification, so dismissing the last one has to leave
  // nothing behind for the footer to keep showing.
  it("is empty again once the last one is dismissed", () => {
    errors.report(refusal("first"));
    errors.report(refusal("second"));

    errors.dismiss();
    errors.dismiss();

    expect(errors.current).toBeNull();
    expect(errors.pending).toBe(0);
  });

  it("ignores a null, which is what a store with no error hands over", () => {
    errors.report(null);
    expect(errors.pending).toBe(0);
  });

  it("does not grow without bound when something fails in a loop", () => {
    for (let i = 0; i < 100; i += 1) errors.report(refusal(`failure ${i}`));
    expect(errors.pending).toBeLessThanOrEqual(20);
    expect(errors.current?.message).toContain("failure 99");
  });

  it("takes a refusal Cogit made on its own, with no Git behind it", () => {
    errors.message("Pick a repository first");
    expect(errors.current?.message).toContain("Pick a repository first");
  });
});
