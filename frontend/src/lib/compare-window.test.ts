import { describe, expect, it, vi } from "vitest";
import type { Whitespace } from "$lib/ipc";
import { firstParent, loadCompare } from "./compare-window";

const request = { repo: 1, path: "src/a.rs", spec: { kind: "workTreeVsIndex" as const } };

// Context lines 10 and Ignore whitespace all in Preferences, and the compare window showed
// three lines of context and every whitespace change.
describe("loadCompare", () => {
  it("loads the diff only once Preferences are read, with their whitespace", async () => {
    let settled: () => void = () => {};
    const settings = {
      current: { ignoreWhitespace: "none" as Whitespace },
      load: vi.fn(
        () =>
          new Promise<void>((resolve) => {
            settled = () => {
              settings.current = { ignoreWhitespace: "all" };
              resolve();
            };
          }),
      ),
    };
    const seen: Whitespace[] = [];
    const diff = {
      whitespace: "none" as Whitespace,
      load: vi.fn(async () => {
        seen.push(diff.whitespace);
      }),
    };

    const loading = loadCompare(request, { settings, diff });
    await Promise.resolve();
    expect(diff.load).not.toHaveBeenCalled();

    settled();
    await loading;
    expect(diff.load).toHaveBeenCalledWith(1, request.spec, "src/a.rs");
    expect(seen).toEqual(["all"]);
  });
});

describe("firstParent", () => {
  const oid = "0123456789abcdef0123456789abcdef01234567";

  it("reads the parent a commit is compared with", async () => {
    const read = vi.fn(async () => ({ parents: ["p1", "p2"] }));

    expect(await firstParent({ ...request, spec: { kind: "commitVsParent", oid } }, read)).toBe("p1");
    expect(read).toHaveBeenCalledWith(1, oid);
  });

  it("is null for a root commit and unknown when the read fails", async () => {
    const spec = { kind: "commitVsParent" as const, oid };

    expect(await firstParent({ ...request, spec }, async () => ({ parents: [] }))).toBeNull();
    expect(
      await firstParent({ ...request, spec }, async () => {
        throw new Error("gone");
      }),
    ).toBeUndefined();
  });

  it("reads nothing for a comparison that has no parent side", async () => {
    const read = vi.fn(async () => ({ parents: ["p1"] }));

    expect(await firstParent(request, read)).toBeUndefined();
    expect(read).not.toHaveBeenCalled();
  });
});
