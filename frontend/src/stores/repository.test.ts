import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = {
  openRepository: vi.fn(),
  closeRepository: vi.fn(),
  repositories: vi.fn(),
  repoStatus: vi.fn(),
  workingState: vi.fn(),
  repoRefs: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { repository } = await import("./repository.svelte");
const { panelView } = await import("$lib/repo-phase");
const { session } = await import("./session.svelte");
const { repoList } = await import("./repo-list.svelte");
const { notices } = await import("./notices.svelte");

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

describe("closing a repository", () => {
  beforeEach(() => {
    commands.openRepository.mockReset();
    commands.closeRepository.mockReset();
    commands.repositories.mockReset();
    commands.repositories.mockResolvedValue({ status: "ok", data: [] });
  });

  // The list came back from a second call after the close: one more round trip, and the
  // frame it lands in was the extra one on screen.
  it("takes the list that is left from the close itself", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/last") });
    await repository.open("C:/repos/last");
    repository.openRepos = [{ repo: summary("C:/repos/last").repo, root: "C:/repos/last" }] as never;
    commands.closeRepository.mockResolvedValue({ status: "ok", data: [] });

    const closing = repository.closeOne(summary("C:/repos/last").repo as never);
    expect(panelView(repository.phase)).toBe("start");
    expect(repository.openRepos).toEqual([]);
    await closing;

    expect(commands.closeRepository).toHaveBeenCalledTimes(1);
    expect(commands.repositories).not.toHaveBeenCalled();
    expect(repository.openRepos).toEqual([]);
  });

  it("still reads the list when the backend refuses the close", async () => {
    commands.closeRepository.mockRejectedValue(new Error("gone"));
    commands.repositories.mockResolvedValue({ status: "ok", data: [{ root: "C:/repos/other" }] });

    await repository.closeOne(7 as never);

    expect(commands.repositories).toHaveBeenCalledTimes(1);
    expect(repository.openRepos.map((entry) => entry.root)).toEqual(["C:/repos/other"]);
  });

  it("lets a list asked for after the close win over the close's own", async () => {
    const closed = pending<unknown>();
    commands.closeRepository.mockReturnValue(closed.promise);
    const closing = repository.closeOne(7 as never);
    commands.repositories.mockResolvedValue({ status: "ok", data: [{ root: "C:/repos/opened" }] });
    await repository.refreshList();

    closed.settle({ status: "ok", data: [] });
    await closing;

    expect(repository.openRepos.map((entry) => entry.root)).toEqual(["C:/repos/opened"]);
  });
});

describe("the status refresh after a mutation", () => {
  beforeEach(() => {
    vi.useRealTimers();
    commands.openRepository.mockReset();
    commands.workingState.mockReset();
    commands.repositories.mockResolvedValue({ status: "ok", data: [] });
    repository.close();
  });

  const state = (conflicted: string[], indexLock: string | null = null) => ({
    status: "ok",
    data: {
      status: { staged: 2, unstaged: 0, untracked: 0, conflicted: conflicted.length },
      conflicted,
      indexLock,
    },
  });

  // The watcher now hears index.lock; the banner has to follow the read it triggers (F-039).
  it("shows an index.lock that appeared and drops one that went", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");

    commands.workingState.mockResolvedValue(state([], "C:/repos/one/.git/index.lock"));
    await repository.refreshStatus();
    expect(repository.current?.indexLock).toBe("C:/repos/one/.git/index.lock");

    commands.workingState.mockResolvedValue(state([]));
    await repository.refreshStatus();
    expect(repository.current?.indexLock).toBeNull();
  });

  // `repo_status` and `conflicted_paths` were two full reads of the same status (R-316).
  it("takes the counters and the conflicted paths from one read", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    commands.workingState.mockResolvedValue(state(["a.txt"]));

    const conflicted = await repository.refreshStatus();

    expect(commands.workingState).toHaveBeenCalledTimes(1);
    expect(commands.repoStatus).not.toHaveBeenCalled();
    expect(repository.current?.status.staged).toBe(2);
    expect(conflicted).toEqual(["a.txt"]);
  });

  it("drops an answer for a repository the panels have left, and says so", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    const answer = pending<unknown>();
    commands.workingState.mockReturnValue(answer.promise);

    const refresh = repository.refreshStatus();
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/second") });
    await repository.open("C:/repos/second");
    answer.settle(state(["a.txt"]));

    expect(await refresh).toBeNull();
    expect(repository.current?.status.staged).toBe(0);
  });

  it("returns nothing when the read fails", async () => {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });
    await repository.open("C:/repos/one");
    commands.workingState.mockRejectedValue(new Error("locked"));

    expect(await repository.refreshStatus()).toBeNull();
  });
});

describe("the refs after a commit", () => {
  beforeEach(() => {
    vi.useRealTimers();
    commands.openRepository.mockReset();
    commands.repoRefs.mockReset();
    commands.repositories.mockResolvedValue({ status: "ok", data: [] });
    repository.close();
  });

  const refs = (oid: string) => ({
    status: "ok",
    data: {
      head: { kind: "branch", name: "master", oid },
      branches: [{ name: "master", oid }],
      tags: [],
      state: { kind: "clean" },
      indexLock: null,
    },
  });

  async function opened(root = "C:/repos/one") {
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary(root) });
    await repository.open(root);
    commands.openRepository.mockClear();
  }

  it("reads the refs into the open repository, without reopening it", async () => {
    await opened();
    const phases: string[] = [];
    commands.repoRefs.mockImplementation(async () => {
      phases.push(repository.phase.kind);
      return refs("b".repeat(40));
    });

    await repository.refreshRefs();

    expect(commands.openRepository).not.toHaveBeenCalled();
    expect(phases).toEqual(["open"]);
    expect(repository.phase.kind).toBe("open");
    expect(repository.current?.head).toEqual({ kind: "branch", name: "master", oid: "b".repeat(40) });
    expect(repository.current?.name).toBe("one");
  });

  it("keeps the counters a later status read brought", async () => {
    await opened();
    const answer = pending<unknown>();
    commands.repoRefs.mockReturnValue(answer.promise);
    commands.workingState.mockResolvedValue({
      status: "ok",
      data: { status: { staged: 0, unstaged: 3, untracked: 0, conflicted: 0 }, conflicted: [] },
    });

    const reading = repository.refreshRefs();
    await repository.refreshStatus();
    answer.settle(refs("b".repeat(40)));
    await reading;

    expect(repository.current?.status.unstaged).toBe(3);
    expect(repository.current?.head).toEqual({ kind: "branch", name: "master", oid: "b".repeat(40) });
  });

  it("drops refs that arrive after the panels moved to another repository", async () => {
    await opened();
    const answer = pending<unknown>();
    commands.repoRefs.mockReturnValue(answer.promise);

    const reading = repository.refreshRefs();
    await opened("C:/repos/second");
    answer.settle(refs("b".repeat(40)));
    await reading;

    expect(repository.current?.root).toBe("C:/repos/second");
    expect(repository.current?.head).toEqual(summary("C:/repos/second").head);
  });

  it("lets the newer of two reads win whichever answers last", async () => {
    await opened();
    const first = pending<unknown>();
    const second = pending<unknown>();
    commands.repoRefs.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const one = repository.refreshRefs();
    const two = repository.refreshRefs();
    second.settle(refs("c".repeat(40)));
    await two;
    first.settle(refs("b".repeat(40)));
    await one;

    expect(repository.current?.head).toEqual({ kind: "branch", name: "master", oid: "c".repeat(40) });
  });

  it("falls back to reopening when the refs cannot be read", async () => {
    await opened();
    commands.repoRefs.mockRejectedValue(new Error("packed-refs locked"));
    commands.openRepository.mockResolvedValue({ status: "ok", data: summary("C:/repos/one") });

    await repository.refreshRefs();

    expect(commands.openRepository).toHaveBeenCalledTimes(1);
    expect(repository.phase.kind).toBe("open");
  });

  it("while an open is in flight, leaves it to the open", async () => {
    await opened();
    const answer = pending<unknown>();
    commands.openRepository.mockReturnValue(answer.promise);
    const reopening = repository.refresh();

    const reading = repository.refreshRefs();
    answer.settle({ status: "ok", data: summary("C:/repos/one") });
    await Promise.all([reopening, reading]);

    expect(commands.repoRefs).not.toHaveBeenCalled();
  });

  const counts = (staged: number, conflicted: string[] = []) => ({
    status: "ok",
    data: { status: { staged, unstaged: 0, untracked: 0, conflicted: conflicted.length }, conflicted },
  });

  // Reads run side by side: the first answer arriving last put back the counters from
  // before the second stage, and its conflicted list went on to the conflicts store.
  it("lets the newer of two status reads win whichever answers last", async () => {
    await opened();
    const first = pending<unknown>();
    const second = pending<unknown>();
    commands.workingState.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const one = repository.refreshStatus();
    const two = repository.refreshStatus();
    second.settle(counts(2));
    await two;
    first.settle(counts(1, ["stale.txt"]));

    expect(await one).toBeNull();
    expect(repository.current?.status.staged).toBe(2);
  });
});

describe("repository store, restoring the last session", () => {
  beforeEach(() => {
    commands.openRepository.mockReset();
    commands.repositories.mockResolvedValue({ status: "ok", data: [] });
    notices.dismissAll();
    for (const root of repoList.list.closed) repoList.forget(root);
  });

  it("reopens every repository the session had, in its order", async () => {
    session.remember(["C:/repos/one", "C:/repos/two"]);
    commands.openRepository.mockImplementation(async (root: string) => ({ status: "ok", data: summary(root) }));

    const failed = await repository.restore();

    expect(failed).toEqual([]);
    expect(commands.openRepository.mock.calls.map((call) => call[0])).toEqual(["C:/repos/one", "C:/repos/two"]);
  });

  it("keeps a folder that did not open as a closed row, and says so", async () => {
    session.remember(["C:/repos/one", "Z:/unmounted"]);
    commands.openRepository.mockImplementation(async (root: string) =>
      root.startsWith("Z:")
        ? { status: "error", error: { kind: "repoNotFound", data: "Z:/unmounted: not found" } }
        : { status: "ok", data: summary(root) },
    );

    const failed = await repository.restore();

    expect(failed).toEqual(["Z:/unmounted"]);
    expect(repoList.list.closed).toContain("Z:/unmounted");
    expect(repoList.list.closed).not.toContain("C:/repos/one");
    const said = notices.all.map((notice) => `${notice.title}\n${notice.body}`).join("\n");
    expect(said).toContain("Z:/unmounted");
  });
});
