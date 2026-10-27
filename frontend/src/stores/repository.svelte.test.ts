import { describe, expect, it, vi } from "vitest";
import { flushSync } from "svelte";

const commands = {
  openRepository: vi.fn(),
  rereadRepository: vi.fn(),
  closeRepository: vi.fn(),
  repositories: vi.fn(),
  workingState: vi.fn(),
  repoRefs: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { repository } = await import("./repository.svelte");

const summary = (root: string) => ({
  repo: root.length,
  root,
  name: root,
  isBare: false,
  head: { kind: "branch", data: { name: "master", oid: "a".repeat(40) } },
  branches: [],
  tags: [],
  status: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0, dirtySubmodules: 0 },
  state: { kind: "clean" },
  indexLock: null,
});

describe("repository.error", () => {
  // A failed open of another folder keeps A on screen; every status re-read replaced the
  // phase with a new object holding the same error, and the effect reporting it ran again.
  it("does not notify again when only the status was re-read", async () => {
    commands.repositories.mockResolvedValue({ status: "ok", data: [] });
    commands.openRepository.mockResolvedValueOnce({ status: "ok", data: summary("A") });
    await repository.open("A");
    commands.openRepository.mockResolvedValueOnce({
      status: "error",
      error: { kind: "invalidState", data: "Not a Git repository" },
    });
    commands.workingState.mockResolvedValue({
      status: "ok",
      data: { status: summary("A").status, conflicted: [], indexLock: null },
    });

    let runs = 0;
    const stop = $effect.root(() => {
      $effect(() => {
        void repository.error;
        runs += 1;
      });
    });
    await repository.open("B");
    flushSync();
    const error = repository.error;
    expect(error).not.toBeNull();
    const before = runs;

    await repository.refreshStatus();
    flushSync();

    expect(runs).toBe(before);
    expect(repository.error).toBe(error);
    stop();
  });
});

describe("re-reading the repository on screen", () => {
  const deferred = <T,>() => {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>((settle) => (resolve = settle));
    return { promise, resolve };
  };

  it("reads once more when the counters changed while it was reading", async () => {
    commands.openRepository.mockResolvedValueOnce({ status: "ok", data: summary("C") });
    await repository.open("C");
    const slow = deferred<unknown>();
    commands.openRepository.mockReturnValueOnce(slow.promise);
    const staged = { ...summary("C").status, staged: 1 };
    commands.rereadRepository.mockResolvedValueOnce({ status: "ok", data: { ...summary("C"), status: staged } });

    const reopened = repository.open("C");
    repository.applyState(summary("C").repo as never, { status: staged, conflicted: [], indexLock: null });
    slow.resolve({ status: "ok", data: summary("C") });
    await reopened;

    expect(commands.rereadRepository).toHaveBeenCalledTimes(1);
    expect(repository.current?.status.staged).toBe(1);
  });

  it("does the refresh asked for while an open of the same folder ran", async () => {
    commands.rereadRepository.mockReset();
    commands.openRepository.mockResolvedValueOnce({ status: "ok", data: summary("D") });
    await repository.open("D");
    const slow = deferred<unknown>();
    commands.openRepository.mockReturnValueOnce(slow.promise);
    commands.rereadRepository.mockResolvedValueOnce({ status: "ok", data: summary("D") });

    const reopened = repository.open("D");
    await repository.refresh();
    slow.resolve({ status: "ok", data: summary("D") });
    await reopened;

    expect(commands.rereadRepository).toHaveBeenCalledTimes(1);
  });
});
