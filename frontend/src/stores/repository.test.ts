import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = {
  openRepository: vi.fn(),
  closeRepository: vi.fn(),
  repositories: vi.fn(),
  repoStatus: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { repository } = await import("./repository.svelte");
const { panelView } = await import("$lib/repo-phase");

const summary = (root: string) => ({
  repo: root.length,
  root,
  name: root.slice(root.lastIndexOf("/") + 1),
  isBare: false,
  head: { kind: "branch", data: { name: "master", oid: "a".repeat(40) } },
  branches: [],
  tags: [],
  status: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 },
  state: { kind: "clean" },
  indexLock: null,
});

/** A promise plus the handles to settle it later, so a test can hold an open in flight. */
function pending<T>() {
  let settle!: (value: T) => void;
  let fail!: (reason: unknown) => void;
  const promise = new Promise<T>((resolve, reject) => {
    settle = resolve;
    fail = reject;
  });
  return { promise, settle, fail };
}

describe("repository store, as a state machine", () => {
  beforeEach(() => {
    vi.useRealTimers();
    commands.openRepository.mockReset();
    commands.repositories.mockResolvedValue({ status: "ok", data: [] });
    repository.close();
  });

  it("starts closed", () => {
    expect(repository.phase.kind).toBe("closed");
    expect(repository.current).toBeNull();
  });

  it("is opening while the backend has not answered", async () => {
    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);

    const open = repository.open("C:/repos/one");

    expect(repository.phase.kind).toBe("opening");
    expect(repository.busy).toBe(true);
    answer.settle({ status: "ok", data: summary("C:/repos/one") });
    await open;
  });

  it("ends up open, with the repository the caller asked for", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });

    await repository.open("C:/repos/one");

    expect(repository.phase.kind).toBe("open");
    expect(repository.current?.root).toBe("C:/repos/one");
    expect(repository.busy).toBe(false);
  });

  it("ends up failed, and stops being busy", async () => {
    commands.openRepository.mockResolvedValue({
      status: "error",
      error: { kind: "invalidState", data: "Not a Git repository" },
    });

    await repository.open("E:/Work/dtv_device");

    expect(repository.phase.kind).toBe("failed");
    expect(repository.busy).toBe(false);
    expect(repository.error?.message).toContain("Not a Git repository");
  });

  // The bug behind "Graph & History (348)" over an empty panel: the watcher fires a
  // refresh, that refresh fails, and the failure used to wipe the repository that was
  // perfectly fine, leaving every panel on its empty state with the counters still full.
  it("keeps the open repository when a later refresh fails", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");

    commands.openRepository.mockResolvedValue({
      status: "error",
      error: { kind: "invalidState", data: "index.lock" },
    });
    await repository.refresh();

    expect(repository.current?.root).toBe("C:/repos/one");
    expect(repository.error?.message).toContain("index.lock");
  });

  it("keeps showing the old repository while a refresh is in flight", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");

    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);
    const refreshing = repository.refresh();

    expect(repository.phase.kind).toBe("opening");
    expect(repository.current?.root).toBe("C:/repos/one");
    answer.settle({ status: "ok", data: summary("C:/repos/one") });
    await refreshing;
  });

  it("lets the newest open win, however the older one ends", async () => {
    const slow = pending<unknown>();
    commands.openRepository.mockReturnValueOnce(slow.promise);
    const first = repository.open("C:/repos/slow");

    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/two") });
    await repository.open("C:/repos/two");

    slow.settle({ status: "ok", data: summary("C:/repos/slow") });
    await first;

    expect(repository.current?.root).toBe("C:/repos/two");
    expect(repository.phase.kind).toBe("open");
  });

  // The watcher answers a `git commit` in the terminal with a refresh. Arriving while the
  // user's click on another repository is opening, it used to reopen the old one and
  // drop the click.
  it("does not let a refresh during an open take the user back", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");

    const clicked = pending<unknown>();
    commands.openRepository.mockReset();
    commands.openRepository.mockReturnValueOnce(clicked.promise);
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    const opening = repository.open("C:/repos/two");
    const refreshing = repository.refresh();

    clicked.settle({ status: "ok", data: summary("C:/repos/two") });
    await Promise.all([opening, refreshing]);

    expect(repository.current?.root).toBe("C:/repos/two");
    expect(commands.openRepository).toHaveBeenCalledTimes(1);
  });

  it("does not let a superseded failure close what is open", async () => {
    const slow = pending<unknown>();
    commands.openRepository.mockReturnValueOnce(slow.promise);
    const first = repository.open("C:/repos/slow");

    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/two") });
    await repository.open("C:/repos/two");

    slow.fail(new Error("too late"));
    await first;

    expect(repository.current?.root).toBe("C:/repos/two");
    expect(repository.error).toBeNull();
  });

  it("gives up waiting rather than spinning for ever", async () => {
    vi.useFakeTimers();
    commands.openRepository.mockReturnValue(pending<unknown>().promise);

    void repository.open("C:/repos/never");
    await vi.advanceTimersByTimeAsync(repository.openTimeoutMs + 1);

    expect(repository.busy).toBe(false);
    expect(repository.phase.kind).toBe("failed");
    expect(repository.error?.message).toMatch(/still/i);
  });

  it("lets a late answer overrule the timeout it already reported", async () => {
    vi.useFakeTimers();
    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);

    const open = repository.open("C:/repos/slow");
    await vi.advanceTimersByTimeAsync(repository.openTimeoutMs + 1);
    answer.settle({ status: "ok", data: summary("C:/repos/slow") });
    await open;

    expect(repository.phase.kind).toBe("open");
    expect(repository.error).toBeNull();
  });

  it("closing goes back to closed, with nothing left behind", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");

    repository.close();

    expect(repository.phase.kind).toBe("closed");
    expect(repository.current).toBeNull();
    expect(repository.error).toBeNull();
    expect(repository.busy).toBe(false);
  });
});

// Work begun for one repository — a watcher cascade, a slow commit hook, a fetch — used
// to finish by reloading panels that by then showed another one.
describe("telling work for a repository the user has left (epoch)", () => {
  beforeEach(() => {
    vi.useRealTimers();
    commands.openRepository.mockReset();
    repository.close();
  });

  it("stays the same across a re-read of the repository on screen", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    const epoch = repository.epoch;

    await repository.refresh();

    expect(repository.epoch).toBe(epoch);
  });

  it("changes as soon as another repository starts opening", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    const epoch = repository.epoch;

    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);
    const opening = repository.open("C:/repos/two");

    expect(repository.epoch).not.toBe(epoch);
    answer.settle({ status: "ok", data: summary("C:/repos/two") });
    await opening;
  });

  it("changes on a switch to a submodule and on close", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    const before = repository.epoch;

    repository.adopt(summary("C:/repos/one/sub") as never);
    const adopted = repository.epoch;
    repository.close();

    expect(adopted).not.toBe(before);
    expect(repository.epoch).not.toBe(adopted);
  });

  // A question or an editor about repository A stayed up while the panels moved to B
  // (a folder dropped on the window, Ctrl+O), and its answer went to B: Discard threw away
  // B's files, Save wrote A's config over B's. Listeners close those at the switch.
  it("tells its listeners the moment the panels leave, and not on a re-read", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    let heard = 0;
    const stop = repository.onLeave(() => (heard += 1));

    await repository.refresh();
    expect(heard).toBe(0);

    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);
    const opening = repository.open("C:/repos/two");
    expect(heard).toBe(1);
    answer.settle({ status: "ok", data: summary("C:/repos/two") });
    await opening;

    stop();
    repository.close();
    expect(heard).toBe(1);
  });

  // `git fetch && git merge` in a terminal: the merge's events arrived while the re-read
  // for the fetch was on its way, were dropped, and Branches kept the old tip.
  it("re-reads once more when the repository changed during a re-read", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    const answer = pending<unknown>();
    commands.openRepository.mockReturnValueOnce(answer.promise);
    commands.openRepository.mockClear();

    const first = repository.refresh();
    await repository.refresh();
    answer.settle({ status: "ok", data: summary("C:/repos/one") });
    await first;

    expect(commands.openRepository).toHaveBeenCalledTimes(2);
  });

  it("tells the caller whose open was overtaken", async () => {
    const slow = pending<unknown>();
    commands.openRepository.mockReturnValueOnce(slow.promise);
    const first = repository.open("C:/repos/slow");
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/two") });
    const second = repository.open("C:/repos/two");

    slow.settle({ status: "ok", data: summary("C:/repos/slow") });

    expect(await first).toBe(false);
    expect(await second).toBe(true);
  });
});

describe("the list of open repositories", () => {
  it("keeps the newer answer when an older one arrives last", async () => {
    const older = pending<unknown>();
    commands.repositories.mockReturnValueOnce(older.promise);
    const first = repository.refreshList();
    commands.repositories.mockResolvedValueOnce({ status: "ok", data: [{ root: "C:/repos/two" }] });
    await repository.refreshList();

    older.settle({ status: "ok", data: [{ root: "C:/repos/closed" }] });
    await first;

    expect(repository.openRepos.map((entry) => entry.root)).toEqual(["C:/repos/two"]);
  });
});

describe("coming back to a listed repository (#50)", () => {
  beforeEach(() => {
    vi.useRealTimers();
    commands.openRepository.mockReset();
    repository.close();
  });

  it("shows what was kept at once, never passing through opening", async () => {
    commands.openRepository.mockResolvedValueOnce({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    repository.keep();
    repository.adopt(summary("C:/repos/one/sub") as never);

    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);
    const back = repository.comeBack("C:/repos/one");

    expect(repository.phase.kind).toBe("open");
    expect(repository.current?.root).toBe("C:/repos/one");
    answer.settle({ status: "ok", data: { ...summary("C:/repos/one"), name: "fresh" } });
    await back;
    expect(repository.phase.kind).toBe("open");
    expect(repository.current?.name).toBe("fresh");
  });

  it("does not let a late re-read undo a newer switch", async () => {
    commands.openRepository.mockResolvedValueOnce({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    repository.keep();

    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);
    const back = repository.comeBack("C:/repos/one");
    repository.adopt(summary("C:/repos/two") as never);
    answer.settle({ status: "ok", data: summary("C:/repos/one") });
    await back;

    expect(repository.current?.root).toBe("C:/repos/two");
  });

  it("opens in the ordinary way when nothing was kept", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/three") });
    await repository.comeBack("C:/repos/three");
    expect(commands.openRepository).toHaveBeenCalledTimes(1);
    expect(repository.current?.root).toBe("C:/repos/three");
  });
});

describe("what the panels see, end to end", () => {
  beforeEach(() => {
    vi.useRealTimers();
    commands.openRepository.mockReset();
    commands.repositories.mockResolvedValue({ status: "ok", data: [] });
    repository.close();
  });

  it("puts every panel on the repository as soon as it opens, unprompted", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });

    await repository.open("C:/repos/one");

    expect(panelView(repository.phase)).toBe("content");
  });

  // The whole of item 1: counters full, panels empty, footer stuck. One failing re-read
  // used to produce all three at once.
  it("does not send the panels back to the start screen when a re-read fails", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");

    commands.openRepository.mockRejectedValue(new Error("the watcher caught it mid-write"));
    await repository.refresh();

    expect(panelView(repository.phase)).toBe("content");
    expect(repository.busy).toBe(false);
  });
});
