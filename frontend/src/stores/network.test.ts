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
    pullRemote: vi.fn(later("pull")),
    pushRemote: vi.fn(later("push")),
  };
});

const { network } = await import("./network.svelte");

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
    const pulling = network.pull(B, "origin", true);
    op("pull:2").finish();
    await pulling;

    expect(network.running).toBe("Fetching");
    expect(network.repo).toBe(A);
    op("fetch:1").finish();
    await fetching;
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
});
