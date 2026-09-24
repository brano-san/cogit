import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = { scanForRepositories: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const { scan } = await import("./scan.svelte");

type Hit = { root: string; name: string; bare: boolean; alreadyOpen: boolean };

const hit = (root: string, alreadyOpen = false): Hit => ({
  root,
  name: root.slice(root.lastIndexOf("/") + 1),
  bare: false,
  alreadyOpen,
});

/** Stands in for the backend: pushes hits through the channel, then resolves. */
function findsBackend(hits: Hit[]) {
  commands.scanForRepositories.mockImplementation((_path, _depth, channel) => {
    for (const found of hits) channel.onmessage(found);
    return Promise.resolve({ status: "ok", data: hits.length });
  });
}

describe("scan store", () => {
  beforeEach(() => {
    commands.scanForRepositories.mockReset();
    scan.clear();
  });

  it("collects the hits the walk reports", async () => {
    findsBackend([hit("/w/alpha"), hit("/w/beta")]);
    await scan.run("/w", 6);
    expect(scan.hits.map((found) => found.name)).toEqual(["alpha", "beta"]);
  });

  it("preselects everything that is not open yet", async () => {
    findsBackend([hit("/w/alpha"), hit("/w/open-one", true)]);
    await scan.run("/w", 6);
    expect([...scan.chosen]).toEqual(["/w/alpha"]);
  });

  it("toggles one repository without touching the rest", async () => {
    findsBackend([hit("/w/alpha"), hit("/w/beta")]);
    await scan.run("/w", 6);
    scan.toggle("/w/alpha");
    expect([...scan.chosen]).toEqual(["/w/beta"]);
  });

  it("never selects one that is already open", async () => {
    findsBackend([hit("/w/open-one", true)]);
    await scan.run("/w", 6);
    scan.toggle("/w/open-one");
    expect(scan.chosen.size).toBe(0);
  });

  it("clears the selection when everything is already selected", async () => {
    findsBackend([hit("/w/alpha"), hit("/w/beta")]);
    await scan.run("/w", 6);
    scan.toggleAll();
    expect(scan.chosen.size).toBe(0);
    scan.toggleAll();
    expect(scan.chosen.size).toBe(2);
  });

  it("is done and idle once the walk finishes", async () => {
    findsBackend([]);
    await scan.run("/w", 6);
    expect(scan.busy).toBe(false);
    expect(scan.done).toBe(true);
  });

  it("reports a failed walk without leaving the dialog busy", async () => {
    commands.scanForRepositories.mockResolvedValue({
      status: "error",
      error: { kind: "internal", data: "gone" },
    });
    await scan.run("/w", 6);
    expect(scan.busy).toBe(false);
    expect(scan.error?.message).toContain("gone");
  });

  it("forgets an earlier walk when a new one starts", async () => {
    findsBackend([hit("/w/alpha")]);
    await scan.run("/w", 6);
    findsBackend([hit("/other/gamma")]);
    await scan.run("/other", 6);
    expect(scan.hits.map((found) => found.name)).toEqual(["gamma"]);
  });
});

// With a filter on, Select All took every hit, the ones the filter hid among them: fifty
// found, two shown, and the button said "Open 50 Repositories".
describe("selecting everything with a filter on", () => {
  beforeEach(() => {
    commands.scanForRepositories.mockReset();
    scan.clear();
  });

  it("acts on the rows shown only", async () => {
    findsBackend([hit("/w/alpha"), hit("/w/beta"), hit("/w/gamma")]);
    await scan.run("/w", 6);

    scan.toggleAll(["/w/alpha"]);
    expect([...scan.chosen].sort()).toEqual(["/w/beta", "/w/gamma"]);

    scan.toggleAll(["/w/alpha"]);
    expect([...scan.chosen].sort()).toEqual(["/w/alpha", "/w/beta", "/w/gamma"]);
  });
});
