import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  repairWorktree: vi.fn(),
  listWorktrees: vi.fn(async () => []),
}));
const inform = vi.hoisted(() => vi.fn());

vi.mock("$lib/ipc", () => ({ CogitError: class extends Error {}, ...ipc }));
vi.mock("$stores/notices.svelte", () => ({ notices: { inform } }));

const { worktrees } = await import("./worktrees.svelte");

const A = 1 as never;

beforeEach(async () => {
  inform.mockReset();
  ipc.repairWorktree.mockReset();
  worktrees.clear();
  await worktrees.refresh(A);
});

describe("Repair", () => {
  it("says in a notification where the worktree was found", async () => {
    ipc.repairWorktree.mockResolvedValue(null);
    await worktrees.repair("E:/Work2/dtv_device_master");

    expect(ipc.repairWorktree).toHaveBeenCalledWith(A, "E:/Work2/dtv_device_master");
    expect(inform).toHaveBeenCalledWith(
      "Worktree repaired",
      "Git knows dtv_device_master is at E:/Work2/dtv_device_master again.",
    );
  });

  it("leaves a failure to the error notice and claims no success", async () => {
    ipc.repairWorktree.mockRejectedValue(new Error("fatal: not a valid path"));
    await expect(worktrees.repair("E:/nowhere")).rejects.toThrow("not a valid path");
    expect(inform).not.toHaveBeenCalled();
  });
});

// The log of a large repository: a watcher burst asked for the list several times at once,
// and every ask was a status of every worktree, ~400 ms each, side by side.
describe("Refresh", () => {
  it("reads once more after the read in flight, however many ask meanwhile", async () => {
    let finish: (value: never[]) => void = () => {};
    ipc.listWorktrees.mockReset();
    ipc.listWorktrees.mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
    ipc.listWorktrees.mockResolvedValue([]);

    const first = worktrees.refresh(A);
    const asks = [worktrees.refresh(A), worktrees.refresh(A), worktrees.refresh(A)];
    expect(ipc.listWorktrees).toHaveBeenCalledTimes(1);

    finish([]);
    await Promise.all([first, ...asks]);
    expect(ipc.listWorktrees).toHaveBeenCalledTimes(2);
  });

  it("reads another repository at once instead of waiting for the one in flight", async () => {
    ipc.listWorktrees.mockReset();
    ipc.listWorktrees.mockImplementationOnce(() => new Promise(() => {}));
    ipc.listWorktrees.mockResolvedValue([]);
    void worktrees.refresh(A);
    await worktrees.refresh(2 as never);
    expect(ipc.listWorktrees).toHaveBeenLastCalledWith(2);
    worktrees.clear();
  });
});
