import { describe, expect, it } from "vitest";
import { applyOperation, busyLabel } from "./operations";

const start = (id: number, label: string) => ({ id, label, success: null });
const end = (id: number, success: boolean) => ({ id, label: "", success });

describe("applyOperation", () => {
  it("records an operation that started", () => {
    expect(applyOperation(new Map(), start(1, "Fetching")).get(1)).toBe("Fetching");
  });

  it("forgets one that finished", () => {
    const running = applyOperation(new Map(), start(1, "Fetching"));
    expect(applyOperation(running, end(1, true)).size).toBe(0);
  });

  it("forgets one that failed just the same", () => {
    const running = applyOperation(new Map(), start(1, "Pushing"));
    expect(applyOperation(running, end(1, false)).size).toBe(0);
  });

  it("keeps the others when one finishes", () => {
    let running = applyOperation(new Map(), start(1, "Fetching"));
    running = applyOperation(running, start(2, "Pushing"));
    expect(applyOperation(running, end(1, true)).get(2)).toBe("Pushing");
  });

  it("ignores an end for something it never saw start", () => {
    expect(applyOperation(new Map(), end(9, true)).size).toBe(0);
  });

  it("does not mutate the map it was given", () => {
    const running = new Map([[1, "Fetching"]]);
    applyOperation(running, end(1, true));
    expect(running.size).toBe(1);
  });
});

describe("busyLabel", () => {
  it("is empty when nothing is running", () => {
    expect(busyLabel(new Map())).toBeNull();
  });

  it("names the single running operation", () => {
    expect(busyLabel(new Map([[1, "Fetching"]]))).toBe("Fetching…");
  });

  it("counts when several run at once", () => {
    const running = new Map([
      [1, "Fetching"],
      [2, "Pushing"],
    ]);
    expect(busyLabel(running)).toBe("2 operations running…");
  });
});
