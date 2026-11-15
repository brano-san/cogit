import { describe, expect, it } from "vitest";
import { moduleMemory } from "./module-memory.svelte";

// A submodule is closed until it is clicked, like a repository at launch: nothing of it is
// read in the background before.
describe("the submodules opened this session", () => {
  it("publishes new sets without changing snapshots held by readers", () => {
    const before = moduleMemory.openModules;
    moduleMemory.openModule("E:/snapshot/lib");
    const opened = moduleMemory.openModules;
    expect(opened).not.toBe(before);
    expect(before.has("E:/snapshot/lib")).toBe(false);
    moduleMemory.openModule("E:/snapshot/lib");
    expect(moduleMemory.openModules).toBe(opened);
    moduleMemory.closeModule("E:/snapshot/lib");
    expect(moduleMemory.openModules).not.toBe(opened);
    expect(opened.has("E:/snapshot/lib")).toBe(true);
    expect(moduleMemory.openModules.has("E:/snapshot/lib")).toBe(false);
  });
  it("start empty and hold what was opened", () => {
    expect(moduleMemory.moduleOpen("E:/w/app/lib")).toBe(false);
    moduleMemory.openModule("E:/w/app/lib");
    expect(moduleMemory.moduleOpen("E:/w/app/lib")).toBe(true);
  });

  it("close one, or every one under a repository that closes", () => {
    moduleMemory.openModule("E:/w/app/lib");
    moduleMemory.openModule("E:/w/app/lib/deep");
    moduleMemory.openModule("E:/w/other/lib");
    moduleMemory.closeModule("E:/w/other/lib");
    expect(moduleMemory.moduleOpen("E:/w/other/lib")).toBe(false);
    moduleMemory.closeModulesUnder("E:/w/app");
    expect([...moduleMemory.openModules]).toEqual([]);
  });
});
