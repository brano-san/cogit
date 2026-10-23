import { beforeEach, describe, expect, it, vi } from "vitest";

// Each network command finishes when the test says so and hands its progress callback over.
const running = vi.hoisted(() => new Map<string, { finish: () => void; line: (text: string) => void }>());

vi.mock("$lib/ipc", () => {
  const later =
    (name: string) =>
    (repo: unknown, _remote: unknown, _flag: unknown, onLine: (text: string) => void) =>
      new Promise<void>((resolve) => running.set(`${name}:${String(repo)}`, { finish: resolve, line: onLine }));
  return {
    fetchRemote: vi.fn((repo: unknown, remote: unknown, onLine: (text: string) => void) =>
      later("fetch")(repo, remote, null, onLine),
    ),
    pullRemote: vi.fn((repo: unknown, remote: unknown, autostash: unknown, onLine: (text: string) => void) =>
      later("pull")(repo, remote, autostash, onLine),
    ),
    pushRemote: vi.fn(later("push")),
    listCredentials: vi.fn(async () => ({ persistent: true, known: [] })),
  };
});

const report = vi.hoisted(() => vi.fn());
vi.mock("$stores/notices.svelte", () => ({ notices: { report } }));
const ask = vi.hoisted(() => vi.fn(async () => true));
vi.mock("$stores/confirm.svelte", () => ({ confirmation: { ask } }));

const ipc = await import("$lib/ipc");
const { network } = await import("./network.svelte");
const { signIn } = await import("./sign-in.svelte");

const A = 1 as never;
const B = 2 as never;

function op(key: string) {
  const found = running.get(key);
  if (!found) throw new Error(`nothing running as ${key}; running: ${[...running.keys()].join(", ")}`);
  return found;
}

beforeEach(() => {
  running.clear();
  network.clear();
});

// One slot served every network operation: the first to finish blanked the status of the
// one still running, and the Exit dialog then saw nothing on the network.
describe("two network operations at once", () => {
  it("keeps showing the one still running when the other finishes", async () => {
    const fetching = network.fetch(A, "origin");
    const pushing = network.push(A, "origin", false);
    op("fetch:1").line("Receiving objects:  40%");

    op("push:1").finish();
    await pushing;

    expect(network.running).toBe("Fetching");
    expect(network.repo).toBe(A);
    expect(network.progress).toBe("Receiving objects:  40%");
    op("fetch:1").finish();
    await fetching;
    expect(network.running).toBeNull();
  });

  it("shows another repository's pull only as its own", async () => {
    const fetching = network.fetch(A, "origin");
    const pulling = network.pull(B, "origin");
    op("pull:2").finish();
    await pulling;

    expect(network.running).toBe("Fetching");
    expect(network.repo).toBe(A);
    op("fetch:1").finish();
    await fetching;
  });
});

describe("a pull with local changes in the way", () => {
  it("asks, and on a yes pulls again with them stashed", async () => {
    vi.mocked(ipc.pullRemote).mockResolvedValueOnce("stashNeeded").mockResolvedValueOnce("pulled");
    ask.mockResolvedValueOnce(true);

    expect(await network.pull(A, "origin")).toBe("pulled");
    expect(ipc.pullRemote).toHaveBeenLastCalledWith(A, "origin", true, expect.any(Function), null);
  });

  it("pulls nothing more on a no", async () => {
    vi.mocked(ipc.pullRemote).mockClear().mockResolvedValueOnce("stashNeeded");
    ask.mockResolvedValueOnce(false);

    expect(await network.pull(A, "origin")).toBe("stashNeeded");
    expect(ipc.pullRemote).toHaveBeenCalledTimes(1);
  });
});

describe("leaving the repository", () => {
  it("stops a running operation's progress from reaching the next one's status bar", async () => {
    const fetching = network.fetch(A, "origin");
    network.clear();

    op("fetch:1").line("Receiving objects:  90%");

    expect(network.progress).toBeNull();
    expect(network.running).toBeNull();
    op("fetch:1").finish();
    await fetching;
  });

  it("keeps a clone's line: it belongs to no repository the panels leave", async () => {
    let finish: (root: string) => void = () => {};
    let line: (text: string) => void = () => {};
    const cloning = network.run(null, "Cloning", (onLine) => {
      line = onLine;
      return new Promise<string>((resolve) => (finish = resolve));
    });
    network.clear();

    line("Receiving objects:  30%");

    expect(network.running).toBe("Cloning");
    expect(network.repo).toBeNull();
    expect(network.progress).toBe("Receiving objects:  30%");
    finish("D:/src/app");
    expect(await cloning).toBe("D:/src/app");
    expect(network.running).toBeNull();
  });
});

describe("a remote that wants a sign-in", () => {
  it("asks in the Sign in dialog and fetches again with what was typed", async () => {
    const refusal = { detail: { kind: "command", data: { stderr: "", stdout: "", authRequired: { host: "h", username: null } } } };
    vi.mocked(ipc.fetchRemote).mockRejectedValueOnce(refusal).mockResolvedValueOnce(null);
    const typed = { host: "h", username: "ann", secret: "pw", remember: true };

    const done = network.fetch(A, "origin");
    await vi.waitFor(() => expect(signIn.open?.host).toBe("h"));
    signIn.answer(typed);
    await done;

    expect(ipc.fetchRemote).toHaveBeenLastCalledWith(A, "origin", expect.any(Function), typed);
    expect(network.failures).toBe(0);
  });
});
