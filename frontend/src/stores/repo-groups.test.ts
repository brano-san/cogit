import { beforeEach, describe, expect, it, vi } from "vitest";
import { UNGROUPED } from "$lib/repo-groups";

const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, value: string) => void store.set(key, value),
  removeItem: (key: string) => void store.delete(key),
});

describe("repository groups", () => {
  beforeEach(() => {
    store.clear();
    vi.resetModules();
  });

  // Requirement: every list in Repositories is closed when Cogit starts (R-160).
  it("starts with every group folded, the ungrouped one included", async () => {
    const { repoGroups } = await import("./repo-groups.svelte");
    repoGroups.add("Work");
    const [id] = repoGroups.groups.order;
    expect(repoGroups.collapsed.has(id!)).toBe(true);
    expect(repoGroups.collapsed.has(UNGROUPED)).toBe(true);
  });

  it("opens a group on the first click and folds it on the second", async () => {
    const { repoGroups } = await import("./repo-groups.svelte");
    repoGroups.add("Work");
    const [id] = repoGroups.groups.order;
    repoGroups.collapse(id!);
    expect(repoGroups.collapsed.has(id!)).toBe(false);
    repoGroups.collapse(id!);
    expect(repoGroups.collapsed.has(id!)).toBe(true);
  });

  it("folds a group made while Cogit runs, the same as one read at start", async () => {
    const { repoGroups } = await import("./repo-groups.svelte");
    repoGroups.add("Later");
    expect(repoGroups.collapsed.has(repoGroups.groups.order.at(-1)!)).toBe(true);
  });
});
