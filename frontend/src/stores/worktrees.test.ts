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
