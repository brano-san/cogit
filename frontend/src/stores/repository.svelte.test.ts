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
  status: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 },
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
