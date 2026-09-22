import { describe, expect, it } from "vitest";
import { recall, remember } from "./session-memory";

describe("session memory", () => {
  it("knows nothing when Cogit has just started", () => {
    expect([...recall("refs", "/w/alpha")]).toEqual([]);
  });

  it("gives back what was remembered for the same list and repository", () => {
    remember("refs", "/w/beta", new Set(["group:local"]));
    expect([...recall("refs", "/w/beta")]).toEqual(["group:local"]);
  });

  it("keeps repositories and lists apart", () => {
    remember("refs", "/w/gamma", new Set(["group:tags"]));
    expect([...recall("refs", "/w/delta")]).toEqual([]);
    expect([...recall("submodules", "/w/gamma")]).toEqual([]);
  });

  it("hands out a copy, so a caller cannot change what is remembered by accident", () => {
    remember("refs", "/w/eps", new Set(["a"]));
    const copy = recall("refs", "/w/eps") as Set<string>;
    copy.add("b");
    expect([...recall("refs", "/w/eps")]).toEqual(["a"]);
  });
});
