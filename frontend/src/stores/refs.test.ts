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
  ...(kind === "group" ? { children: true } : {}),
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

  // Requirement 3: a repository opened for the first time shows its headings folded.
  it("folds every heading of a repository it has never seen", () => {
    refs.adopt("/w/alpha", TREE);
    expect([...refs.collapsed].sort()).toEqual(["group:local", "group:tags"]);
  });

  it("brings back the headings the user opened, and only those", () => {
    refs.adopt("/w/alpha", TREE);
    refs.collapse("group:tags");

    refs.clear();
    refs.adopt("/w/alpha", TREE);
    expect([...refs.collapsed]).toEqual(["group:local"]);
  });

  it("folds a heading that only appears later, the way Stashes does once they load", () => {
    refs.adopt("/w/alpha", TREE);
    refs.collapse("group:local");
    refs.know([...TREE, node("group:stashes", "group")]);
    expect([...refs.collapsed].sort()).toEqual(["group:stashes", "group:tags"]);
  });

  it("starts a second repository folded however the first one was left", () => {
    refs.adopt("/w/alpha", TREE);
    refs.collapse("group:local");
    refs.clear();

    refs.adopt("/w/beta", TREE);
    expect([...refs.collapsed].sort()).toEqual(["group:local", "group:tags"]);
  });

  /** The list used to hold what was folded; a repository saved that way keeps its looks. */
  it("reads a repository saved before the change the way it was left", () => {
    store.set(
      "cogit.visible-refs.v2",
      JSON.stringify({ "/w/alpha": { visible: ["HEAD"], collapsed: ["group:tags"] } }),
    );
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

  it("remembers a heading folded again after it was opened", () => {
    refs.adopt("/w/alpha", TREE);
    refs.collapse("group:local");
    refs.collapse("group:local");

    refs.clear();
    refs.adopt("/w/alpha", TREE);
    expect([...refs.collapsed].sort()).toEqual(["group:local", "group:tags"]);
  });

  it("ignores a stored entry that is not the shape it wrote", () => {
    store.set("cogit.visible-refs.v2", JSON.stringify({ "/w/alpha": { visible: 7 } }));
    refs.adopt("/w/alpha", TREE);
    expect([...refs.visible].sort()).toEqual(["HEAD", "local:master", "local:topic"]);
    expect([...refs.collapsed].sort()).toEqual(["group:local", "group:tags"]);
  });

  it("survives a corrupt store rather than refusing to open the panel", () => {
    store.set("cogit.visible-refs.v2", "{not json");
    refs.adopt("/w/alpha", TREE);
    expect(refs.visible.size).toBeGreaterThan(0);
  });
});
