import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/ipc", () => ({ safetyLog: vi.fn(), undoEntry: vi.fn(), undoLast: vi.fn() }));

const { safety } = await import("./safety.svelte");

const A = 1 as never;
const B = 2 as never;

// The toolbar asked `last !== undefined` of a getter that answers null, so Undo was
// always on; and `last` was the newest entry of any repository, so in A the button
// offered, and undid in A, what had been done in B.
describe("what Undo offers", () => {
  it("is the newest undoable entry of the repository on screen, or nothing", () => {
    safety.entries = [
      { id: 3, repo: B, description: "Discard b.txt", undoable: true },
      { id: 2, repo: A, description: "Checkout main", undoable: false },
      { id: 1, repo: A, description: "Discard a.txt", undoable: true },
    ] as never;

    expect(safety.lastFor(A)?.description).toBe("Discard a.txt");
    expect(safety.lastFor(null)).toBeNull();
    safety.entries = [];
    expect(safety.lastFor(A)).toBeNull();
  });
});
