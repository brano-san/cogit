import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands: {}, events: {} }));

const { placesShown, PLACES_SHOWN, pushError } = await import("./notices");

const places = (count: number) =>
  Array.from({ length: count }, (_, i) => ({ label: `dtv_device [import/m${i}]` }));

const error = (key: string, body: string, record?: number) => ({
  key,
  severity: "error" as const,
  title: "Push failed",
  body,
  report: body,
  repeats: 1,
  ...(record === undefined ? {} : { record }),
});

describe("placesShown", () => {
  it("shows a short list whole", () => {
    expect(placesShown(places(3), false)).toEqual({ shown: places(3), more: 0 });
  });

  // Twenty-one repositories with one problem: the first few, then a count.
  it("folds a long list after the first few into `and N more`", () => {
    const { shown, more } = placesShown(places(21), false);
    expect(shown).toHaveLength(PLACES_SHOWN);
    expect(more).toBe(21 - PLACES_SHOWN);
  });

  it("never says `and 1 more`: one row is cheaper than a button", () => {
    expect(placesShown(places(PLACES_SHOWN + 1), false).more).toBe(0);
  });

  it("shows every place once opened", () => {
    expect(placesShown(places(21), true).shown).toHaveLength(21);
  });
});

describe("pushError", () => {
  it("does not queue the same record twice", () => {
    const once = pushError([], error("command:7", "rejected", 7));
    expect(pushError(once, error("command:7", "rejected", 7))).toHaveLength(1);
  });

  it("moves a repeated failure to the front and counts it", () => {
    const queue = [error("command:1", "other", 1), error("command:7", "rejected", 7)];
    const next = pushError(queue, error("command:8", "rejected", 8));
    expect(next.map((notice) => notice.key)).toEqual(["command:8", "command:1"]);
    expect(next[0]?.repeats).toBe(2);
  });
});
