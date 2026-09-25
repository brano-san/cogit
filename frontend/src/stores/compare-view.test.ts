import { beforeEach, describe, expect, it, vi } from "vitest";

const shown = vi.hoisted(() => ({ path: null as string | null, spec: null as unknown }));

vi.mock("$lib/ipc/ref-ops", () => ({ compareFiles: vi.fn(async () => []) }));
vi.mock("$stores/diff.svelte", () => ({
  diff: {
    get path() {
      return shown.path;
    },
    shows: (spec: { kind: string; a?: string; b?: string }, path: string) =>
      shown.path === path && JSON.stringify(shown.spec) === JSON.stringify(spec),
    clear: vi.fn(() => {
      shown.path = null;
    }),
    load: vi.fn(async (_repo: unknown, spec: unknown, path: string) => {
      shown.path = path;
      shown.spec = spec;
    }),
  },
}));

const { compareView } = await import("./compare-view.svelte");
const { diff } = await import("$stores/diff.svelte");

const REPO = 1 as never;

beforeEach(async () => {
  vi.mocked(diff.clear).mockClear();
  vi.mocked(diff.load).mockClear();
  shown.path = null;
  shown.spec = null;
  await compareView.show(REPO, "aaa", "bbb");
});

// The second click compared the path alone: the same file open in Diff under the commit's
// own diff made the click in the compare list blank the panel instead of comparing.
describe("clicking a file in the compare list", () => {
  it("shows the comparison when the same file is open under another diff", () => {
    shown.path = "x.txt";
    shown.spec = { kind: "commitVsParent", oid: "bbb" };

    compareView.open("x.txt");

    expect(diff.clear).not.toHaveBeenCalled();
    expect(diff.load).toHaveBeenCalledWith(REPO, { kind: "commitVsCommit", a: "aaa", b: "bbb" }, "x.txt");
  });

  it("closes the comparison on a second click", () => {
    compareView.open("x.txt");
    compareView.open("x.txt");

    expect(diff.clear).toHaveBeenCalledOnce();
  });
});
