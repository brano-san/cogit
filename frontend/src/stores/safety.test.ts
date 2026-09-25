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

// Undo showed one entry in its tooltip and undid whichever was newest when its turn came
// in the queue: a Discard queued behind a push was undone instead of the Delete branch
// the tooltip named, and a double-click undid two.
describe("Undo in the toolbar", () => {
  it("undoes the entry it shows, by its id", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.undoEntry).mockResolvedValue({} as never);
    vi.mocked(ipc.safetyLog).mockResolvedValue([]);
    safety.entries = [
      { id: 7, repo: A, description: "Delete branch foo", undoable: true },
      { id: 6, repo: A, description: "Discard a.txt", undoable: true },
    ] as never;

    await safety.undoShown(A);

    expect(ipc.undoEntry).toHaveBeenCalledWith(A, 7);
    expect(ipc.undoLast).not.toHaveBeenCalled();
  });

  it("undoes once for a second click while the first is on its way", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.undoEntry).mockClear();
    let done: () => void = () => {};
    vi.mocked(ipc.undoEntry).mockReturnValueOnce(new Promise((resolve) => (done = () => resolve({} as never))));
    vi.mocked(ipc.safetyLog).mockResolvedValue([]);
    safety.entries = [{ id: 7, repo: A, description: "Delete branch foo", undoable: true }] as never;

    const first = safety.undoShown(A);
    await safety.undoShown(A);
    done();
    await first;

    expect(ipc.undoEntry).toHaveBeenCalledOnce();
  });

  it("does nothing when there is nothing to undo", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.undoEntry).mockClear();
    safety.entries = [];

    expect(await safety.undoShown(A)).toBeNull();
    expect(ipc.undoEntry).not.toHaveBeenCalled();
  });
});
