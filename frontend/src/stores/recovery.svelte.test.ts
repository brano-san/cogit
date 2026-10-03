import { beforeEach, describe, expect, it, vi } from "vitest";

type Take = (rows: unknown[]) => void;
const streams = vi.hoisted(() => [] as { take: Take; done: () => void }[]);

vi.mock("$lib/ipc", () => ({
  lostCommits: vi.fn(
    (_repo: unknown, take: Take) =>
      new Promise<void>((resolve) => streams.push({ take, done: resolve })),
  ),
}));

const { recovery } = await import("./recovery.svelte");

const at = (i: number) => streams[i] as { take: Take; done: () => void };
const row = (oid: string) => ({ oid }) as never;

beforeEach(() => {
  streams.length = 0;
  recovery.clear();
});

describe("the lost commits arrive in chunks", () => {
  it("shows the first chunk before the last one is in, and all of them at the end", async () => {
    const read = recovery.refresh(1 as never);
    at(0).take([row("a"), row("b")]);
    expect(recovery.lost).toEqual([row("a"), row("b")]);

    at(0).take([row("c")]);
    at(0).done();
    await read;

    expect(recovery.lost.map((r) => r.oid)).toEqual(["a", "b", "c"]);
  });

  it("replaces the previous list instead of appending to it", async () => {
    const first = recovery.refresh(1 as never);
    at(0).take([row("old")]);
    at(0).done();
    await first;

    const second = recovery.refresh(1 as never);
    at(1).take([row("new")]);
    at(1).done();
    await second;

    expect(recovery.lost.map((r) => r.oid)).toEqual(["new"]);
  });

  // A refresh per disk event restarted the scan each time, seconds each on a cold disk.
  it("reads once more after the running read, not once per refresh", async () => {
    const reads = [1, 2, 3].map(() => recovery.refresh(1 as never));
    expect(streams.length).toBe(1);

    at(0).take([row("stale")]);
    at(0).done();
    await vi.waitFor(() => expect(streams.length).toBe(2));
    at(1).take([row("fresh")]);
    at(1).done();
    await Promise.all(reads);

    expect(streams.length).toBe(2);
    expect(recovery.lost.map((r) => r.oid)).toEqual(["fresh"]);
  });

  it("drops the chunks of the repository left", async () => {
    const left = recovery.refresh(1 as never);
    recovery.clear();
    const opened = recovery.refresh(2 as never);
    expect(streams.length).toBe(2);

    at(1).take([row("opened")]);
    at(0).take([row("left")]);
    at(0).done();
    at(1).done();
    await Promise.all([left, opened]);

    expect(recovery.lost.map((r) => r.oid)).toEqual(["opened"]);
  });
});
