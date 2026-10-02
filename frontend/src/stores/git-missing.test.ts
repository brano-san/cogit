import { describe, expect, it, vi } from "vitest";
import type { Probe } from "$lib/git-check";
import { GitMissingStore, type GitMissingDeps } from "./git-missing.svelte";

const works = (version: string, olderThan: string | null = null): Probe => ({ valid: true, version, error: null, olderThan });
const broken = (error: string): Probe => ({ valid: false, version: null, error, olderThan: null });

function make(probe: (path: string) => Probe | Promise<Probe>, found = [{ path: "C:/Git/cmd/git.exe", version: "2.50.1" }]) {
  const deps = {
    probe: vi.fn(async (path: string) => probe(path)),
    candidates: vi.fn(async () => found),
    apply: vi.fn(async () => {}),
    save: vi.fn(async () => {}),
  } satisfies GitMissingDeps;
  return { store: new GitMissingStore(deps), deps };
}

describe("GitMissingStore startup check", () => {
  it("stays quiet when git works", async () => {
    const { store } = make(() => works("2.50.1"));
    await store.check("");
    expect(store.open).toBeNull();
    expect(store.missing).toBe(false);
  });

  it("opens once with the reason and lists the candidates", async () => {
    const { store, deps } = make(() => broken("cannot run git: not found"));
    await store.check("");
    expect(store.open).toEqual({ kind: "missing", reason: "cannot run git: not found" });
    expect(store.missing).toBe(true);
    await vi.waitFor(() => expect(store.candidates).toHaveLength(1));
    store.continueWithout();
    await store.check("");
    expect(store.open).toBeNull();
    expect(store.missing).toBe(true);
    expect(deps.candidates).toHaveBeenCalledTimes(1);
  });

  it("treats a probe that throws as a missing git", async () => {
    const { store } = make(() => Promise.reject(new Error("ipc down")));
    await store.check("x");
    expect(store.open).toEqual({ kind: "missing", reason: "ipc down" });
  });

  it("warns once about an old git without marking it missing", async () => {
    const { store } = make(() => works("2.41.0", "2.45"));
    await store.check("");
    expect(store.open).toEqual({ kind: "old", version: "2.41.0", minimum: "2.45" });
    expect(store.missing).toBe(false);
    store.continueWithout();
    await store.check("");
    expect(store.open).toBeNull();
  });
});

describe("GitMissingStore choose", () => {
  it("applies, saves and closes on a working git", async () => {
    const { store, deps } = make((path) => (path === "D:/g/git.exe" ? works("2.50.1") : broken("no")));
    await store.check("");
    expect(await store.choose("D:/g/git.exe")).toBe(true);
    expect(deps.apply).toHaveBeenCalledWith("D:/g/git.exe");
    expect(deps.save).toHaveBeenCalledWith("D:/g/git.exe");
    expect(store.open).toBeNull();
    expect(store.missing).toBe(false);
  });

  it("keeps the dialog and shows why a pick does not work", async () => {
    const { store, deps } = make(() => broken("x is not git"));
    await store.check("");
    expect(await store.choose("D:/notepad.exe")).toBe(false);
    expect(store.picked).toEqual({ state: "bad", path: "D:/notepad.exe", reason: "x is not git" });
    expect(store.open).not.toBeNull();
    expect(deps.save).not.toHaveBeenCalled();
  });

  it("drops the answer of a pick that a newer pick overtook", async () => {
    let release: (p: Probe) => void = () => {};
    const { store, deps } = make((path) =>
      path === "slow" ? new Promise<Probe>((resolve) => (release = resolve)) : works("2.50.1"),
    );
    const first = store.choose("slow");
    expect(await store.choose("fast")).toBe(true);
    release(works("2.30.0"));
    expect(await first).toBe(false);
    expect(deps.apply).toHaveBeenCalledTimes(1);
  });
});

describe("GitMissingStore reopen", () => {
  it("shows the dialog again after Continue without git, with fresh candidates", async () => {
    const { store, deps } = make(() => broken("gone"));
    await store.check("");
    store.continueWithout();
    store.reopen("cannot run git: not found");
    expect(store.open).toEqual({ kind: "missing", reason: "cannot run git: not found" });
    await vi.waitFor(() => expect(deps.candidates).toHaveBeenCalledTimes(2));
  });
});
