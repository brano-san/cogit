import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CiTarget, RepoId } from "$lib/ipc";

const commands = { ciStatuses: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { ciStatus } = await import("./ci-status.svelte");

const repo = 1 as unknown as RepoId;
const other = 2 as unknown as RepoId;

function rows(...shas: string[]): CiTarget[] {
  return shas.map((sha) => ({ sha, remote: null }));
}

function answer(retryMs: number | null, ...shas: string[]) {
  return {
    status: "ok",
    data: {
      statuses: shas.map((sha) => ({ sha, state: "pending", checks: [], url: null, note: null })),
      retryMs,
    },
  };
}

function asked(call: number): string[] {
  return (commands.ciStatuses.mock.calls[call]![1] as CiTarget[]).map((target) => target.sha);
}

beforeEach(() => {
  vi.useFakeTimers();
  commands.ciStatuses.mockReset();
  ciStatus.clear();
});

afterEach(() => {
  ciStatus.clear();
  vi.useRealTimers();
});

describe("ciStatus", () => {
  it("asks for the window where the scroll stopped, not every frame of it", async () => {
    commands.ciStatuses.mockResolvedValue(answer(null));
    ciStatus.setWindow(repo, "graph", rows("a", "b"));
    ciStatus.setWindow(repo, "graph", rows("b", "c"));
    ciStatus.setWindow(repo, "graph", rows("c", "d"));
    await vi.advanceTimersByTimeAsync(300);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
    expect(asked(0)).toEqual(["c", "d"]);
  });

  it("asks for the branches beside the graph rows, each commit once", async () => {
    commands.ciStatuses.mockResolvedValue(answer(null));
    ciStatus.setWindow(repo, "graph", rows("a", "b"));
    ciStatus.setWindow(repo, "branches", [{ sha: "b", remote: "up" }, { sha: "t", remote: "up" }]);
    await vi.advanceTimersByTimeAsync(300);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
    expect(asked(0)).toEqual(["a", "b", "t"]);
  });

  it("the same window is not asked again unless a poll is due", async () => {
    commands.ciStatuses.mockResolvedValue(answer(null, "a"));
    ciStatus.setWindow(repo, "graph", rows("a"));
    await vi.advanceTimersByTimeAsync(300);
    ciStatus.dropWindow("graph");
    ciStatus.setWindow(repo, "graph", rows("a"));
    await vi.advanceTimersByTimeAsync(60_000);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
    expect(ciStatus.look("a")?.state).toBe("pending");
  });

  it("polls only while the backend says something is still running, with the rows on screen then", async () => {
    commands.ciStatuses.mockResolvedValueOnce(answer(15_000, "a")).mockResolvedValue(answer(null, "x"));
    ciStatus.setWindow(repo, "graph", rows("a"));
    await vi.advanceTimersByTimeAsync(300);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(14_000);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(2);
    expect(asked(1)).toEqual(["a"]);
    await vi.advanceTimersByTimeAsync(600_000);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(2);
  });

  it("a scroll during a poll's wait asks for the new rows at once and drops the old poll", async () => {
    commands.ciStatuses.mockResolvedValueOnce(answer(15_000, "a")).mockResolvedValue(answer(null));
    ciStatus.setWindow(repo, "graph", rows("a"));
    await vi.advanceTimersByTimeAsync(300);
    ciStatus.setWindow(repo, "graph", rows("z"));
    await vi.advanceTimersByTimeAsync(300);
    expect(asked(1)).toEqual(["z"]);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(2);
  });

  it("an answer for a repository left behind is dropped", async () => {
    let finish: (value: unknown) => void = () => {};
    commands.ciStatuses.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
    ciStatus.setWindow(repo, "graph", rows("a"));
    await vi.advanceTimersByTimeAsync(300);
    ciStatus.setWindow(other, "graph", []);
    finish(answer(15_000, "a"));
    await vi.advanceTimersByTimeAsync(60_000);
    expect(ciStatus.look("a")).toBeUndefined();
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
  });

  it("a failed question is not repeated in a loop", async () => {
    commands.ciStatuses.mockRejectedValue(new Error("down"));
    ciStatus.setWindow(repo, "graph", rows("a"));
    await vi.advanceTimersByTimeAsync(600_000);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
  });

  it("turned off: nothing is held and nothing is polled", async () => {
    commands.ciStatuses.mockResolvedValue(answer(15_000, "a"));
    ciStatus.setWindow(repo, "graph", rows("a"));
    await vi.advanceTimersByTimeAsync(300);
    ciStatus.clear();
    expect(ciStatus.statuses.size).toBe(0);
    await vi.advanceTimersByTimeAsync(600_000);
    expect(commands.ciStatuses).toHaveBeenCalledTimes(1);
  });
});
