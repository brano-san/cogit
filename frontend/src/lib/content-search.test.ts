import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FileEntry, RepoId, SearchChunk } from "./ipc/bindings";
import { DEFAULT_VIEW } from "./file-view";

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands: {} }));

const { ContentSearch, SETTLE_MS, contentQuery, contentScope, keepFile } = await import(
  "./content-search.svelte"
);

const REPO = 7 as unknown as RepoId;

function entry(path: string): FileEntry {
  return { path, oldPath: null, status: "modified", mode: "plain", modeChange: null, similarity: null };
}

/** A search the test drives by hand: it resolves when the test says the backend is done. */
function backend() {
  const runs: { query: string; emit: (chunk: SearchChunk) => void; finish: () => void; fail: (e: Error) => void }[] = [];
  const run = vi.fn(
    (_repo: RepoId, query: { text: string }, onChunk: (chunk: SearchChunk) => void) =>
      new Promise<void>((resolve, reject) => {
        runs.push({ query: query.text, emit: onChunk, finish: resolve, fail: reject });
      }),
  );
  const cancel = vi.fn(async () => true);
  return { runs, run, cancel };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("contentQuery", () => {
  it("is off while the switch is off", () => {
    expect(contentQuery({ ...DEFAULT_VIEW, contents: false }, "needle")).toBeNull();
  });

  it("is off while the field is empty", () => {
    expect(contentQuery({ ...DEFAULT_VIEW, contents: true }, "   ")).toBeNull();
  });

  it("carries the text, the regex switch and the scope", () => {
    expect(contentQuery({ ...DEFAULT_VIEW, contents: true, regex: true }, " a.b ")).toEqual({
      text: "a.b",
      regex: true,
      scope: "changed",
    });
  });
});

describe("contentScope", () => {
  it("reads every file once unchanged files are listed too", () => {
    expect(contentScope({ ...DEFAULT_VIEW, unchanged: true })).toBe("all");
    expect(contentScope({ ...DEFAULT_VIEW, unchanged: false })).toBe("changed");
  });
});

describe("keepFile", () => {
  const pattern = { test: (subject: string) => subject.includes("readme"), broken: false };

  it("filters by name while no content search is active", () => {
    expect(keepFile(entry("docs/readme.md"), pattern, null)).toBe(true);
    expect(keepFile(entry("src/main.rs"), pattern, null)).toBe(false);
  });

  it("keeps only files with a match once contents are searched, whatever their name", () => {
    const hits = new Map([["src/main.rs", 2]]);
    expect(keepFile(entry("src/main.rs"), pattern, hits)).toBe(true);
    expect(keepFile(entry("docs/readme.md"), pattern, hits)).toBe(false);
  });
});

describe("ContentSearch", () => {
  const QUERY = { text: "needle", regex: false, scope: "changed" as const };

  it("waits for the typing pause before asking the backend", () => {
    const { run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);

    search.set(QUERY);
    expect(search.busy).toBe(true);
    expect(run).not.toHaveBeenCalled();

    vi.advanceTimersByTime(SETTLE_MS);
    expect(run).toHaveBeenCalledWith(REPO, QUERY, expect.any(Function));
  });

  it("counts matching lines per file as the chunks arrive", async () => {
    const { runs, run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);

    const [first] = runs;
    first!.emit({ kind: "started", id: 1 });
    first!.emit({
      kind: "matches",
      matches: [
        { path: "a.txt", line: 1, preview: "needle" },
        { path: "a.txt", line: 9, preview: "needle again" },
        { path: "b.txt", line: 3, preview: "a needle" },
      ],
    });
    expect(search.hits).toEqual(new Map([["a.txt", 2], ["b.txt", 1]]));

    first!.emit({ kind: "done", total: 3, cancelled: false });
    first!.finish();
    await vi.runAllTimersAsync();
    expect(search.busy).toBe(false);
  });

  it("reports an empty result as no files rather than as no search", async () => {
    const { runs, run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);

    runs[0]!.emit({ kind: "started", id: 1 });
    runs[0]!.emit({ kind: "done", total: 0, cancelled: false });

    expect(search.hits).toEqual(new Map());
  });

  it("cancels the running search when the query changes and ignores what it still sends", () => {
    const { runs, run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);
    runs[0]!.emit({ kind: "started", id: 41 });

    search.set({ ...QUERY, text: "needles" });
    expect(cancel).toHaveBeenCalledWith(41);

    runs[0]!.emit({ kind: "matches", matches: [{ path: "stale.txt", line: 1, preview: "" }] });
    vi.advanceTimersByTime(SETTLE_MS);
    runs[1]!.emit({ kind: "started", id: 42 });
    runs[1]!.emit({ kind: "matches", matches: [{ path: "fresh.txt", line: 1, preview: "" }] });

    expect(search.hits).toEqual(new Map([["fresh.txt", 1]]));
  });

  it("cancels a stale search that only reports its id after being replaced", () => {
    const { runs, run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);

    search.set({ ...QUERY, text: "other" });
    runs[0]!.emit({ kind: "started", id: 5 });

    expect(cancel).toHaveBeenCalledWith(5);
  });

  it("does not search again for the same query", () => {
    const { run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);
    search.set({ ...QUERY });
    vi.advanceTimersByTime(SETTLE_MS);

    expect(run).toHaveBeenCalledOnce();
  });

  it("goes back to name filtering when the query is cleared", () => {
    const { runs, run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);
    runs[0]!.emit({ kind: "started", id: 3 });

    search.set(null);

    expect(cancel).toHaveBeenCalledWith(3);
    expect(search.hits).toBeNull();
    expect(search.busy).toBe(false);
  });

  it("shows the backend's refusal, for a pattern it cannot compile", async () => {
    const { runs, run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);

    runs[0]!.fail(new Error("regex parse error"));
    await vi.runAllTimersAsync();

    expect(search.error).toBe("regex parse error");
    expect(search.hits).toEqual(new Map());
    expect(search.busy).toBe(false);
  });

  it("asks the same query again when the files on disk changed", () => {
    const { run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);
    search.set(QUERY);
    vi.advanceTimersByTime(SETTLE_MS);

    search.refresh();
    vi.advanceTimersByTime(SETTLE_MS);

    expect(run).toHaveBeenCalledTimes(2);
  });

  it("does nothing on refresh while no query is set", () => {
    const { run, cancel } = backend();
    const search = new ContentSearch(() => REPO, run, cancel);

    search.refresh();
    vi.advanceTimersByTime(SETTLE_MS);

    expect(run).not.toHaveBeenCalled();
  });
});
