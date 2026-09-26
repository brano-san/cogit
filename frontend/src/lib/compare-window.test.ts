import { describe, expect, it, vi } from "vitest";
import type { Whitespace } from "$lib/ipc";
import { loadCompare } from "./compare-window";

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
