import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RefNode } from "$lib/ref-nodes";

const commands = { remotes: vi.fn(), remoteUrl: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

/** The store has to survive a browser that refuses storage, so it is stubbed, not faked. */
const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, value: string) => void store.set(key, value),
  removeItem: (key: string) => void store.delete(key),
});

const { refs } = await import("./refs.svelte");

const node = (id: string, kind: RefNode["kind"], rev?: string): RefNode => ({
  id,
  kind,
  label: id,
  depth: kind === "group" ? 0 : 1,
  rev,
});

const TREE: RefNode[] = [
  node("HEAD", "head", "HEAD"),
  node("group:local", "group"),
  node("local:master", "local", "refs/heads/master"),
  node("local:topic", "local", "refs/heads/topic"),
  node("group:tags", "group"),
  node("tag:v1", "tag", "refs/tags/v1"),
];

describe("refs store", () => {
  beforeEach(() => {
    store.clear();
    refs.clear();
  });

  it("starts a fresh repository with HEAD and the local branches ticked", () => {
    refs.adopt("/w/alpha", TREE);
    expect([...refs.visible].sort()).toEqual(["HEAD", "local:master", "local:topic"]);
  });

  it("brings back the ticks the user left", () => {
    refs.adopt("/w/alpha", TREE);
    refs.set(new Set(["tag:v1"]));

    refs.clear();
    refs.adopt("/w/alpha", TREE);
    expect([...refs.visible]).toEqual(["tag:v1"]);
  });

  it("brings back the folded headings too", () => {
    refs.adopt("/w/alpha", TREE);
    refs.collapse("group:tags");

    refs.clear();
    refs.adopt("/w/alpha", TREE);
    expect([...refs.collapsed]).toEqual(["group:tags"]);
  });

  it("keeps each repository's state apart", () => {
    refs.adopt("/w/alpha", TREE);
    refs.set(new Set(["tag:v1"]));
    refs.clear();

    refs.adopt("/w/beta", TREE);
    expect([...refs.visible].sort()).toEqual(["HEAD", "local:master", "local:topic"]);
  });

  it("drops a remembered tick for a ref that is gone", () => {
    refs.adopt("/w/alpha", TREE);
    refs.set(new Set(["local:master", "local:gone"]));

    refs.clear();
    refs.adopt("/w/alpha", TREE);
    expect([...refs.visible]).toEqual(["local:master"]);
  });

  it("falls back to the defaults when every remembered ref is gone", () => {
    refs.adopt("/w/alpha", TREE);
    refs.set(new Set(["local:gone"]));

    refs.clear();
    refs.adopt("/w/alpha", TREE);
    expect([...refs.visible].sort()).toEqual(["HEAD", "local:master", "local:topic"]);
  });

  it("remembers that the user unticked everything", () => {
    refs.adopt("/w/alpha", TREE);
    refs.collapse("group:local");

    refs.clear();
    refs.adopt("/w/alpha", TREE);
    expect([...refs.collapsed]).toEqual(["group:local"]);
  });

  it("ignores a stored entry that is not the shape it wrote", () => {
    store.set("cogit.visible-refs.v2", JSON.stringify({ "/w/alpha": { visible: 7 } }));
    refs.adopt("/w/alpha", TREE);
    expect([...refs.visible].sort()).toEqual(["HEAD", "local:master", "local:topic"]);
    expect([...refs.collapsed]).toEqual([]);
  });

  it("survives a corrupt store rather than refusing to open the panel", () => {
    store.set("cogit.visible-refs.v2", "{not json");
    refs.adopt("/w/alpha", TREE);
    expect(refs.visible.size).toBeGreaterThan(0);
  });
});
