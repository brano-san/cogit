import { describe, expect, it } from "vitest";
import { moduleMemory } from "./module-memory.svelte";

// A submodule is closed until it is clicked, like a repository at launch: nothing of it is
// read in the background before.
describe("the submodules opened this session", () => {
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
