import { describe, expect, it, vi } from "vitest";
import { ModuleInitialiser, moduleClick } from "./module-init";

// One click on a submodule that was never checked out ran `git submodule update --init`
// without a word, and a double-click ran it three times.
describe("a click on a submodule", () => {
  it("offers to initialise one that is not checked out, and opens the rest", () => {
    expect(moduleClick("notInitialised")).toBe("offer");
    for (const state of ["inSync", "behind", "ahead", "diverged", "unknown", "unread"] as const) {
      expect(moduleClick(state)).toBe("open");
    }
  });

  // A second click on the submodule on screen reopened it: the selected commit and the Diff
  // went, the graph and the worktrees were read again; a double-click to expand did it thrice.
  it("reloads nothing for the submodule on screen", () => {
    expect(moduleClick("inSync", { key: "lib/a", shown: "lib/a", opening: null })).toBe("stay");
  });

  it("does not start a second open while the first is under way", () => {
    expect(moduleClick("inSync", { key: "lib/a", shown: null, opening: "lib/a" })).toBe("stay");
  });

  it("opens another submodule, and this one again once the panels moved on", () => {
    expect(moduleClick("inSync", { key: "lib/b", shown: "lib/a", opening: null })).toBe("open");
    expect(moduleClick("inSync", { key: "lib/a", shown: null, opening: null })).toBe("open");
  });
});

describe("initialising a submodule", () => {
  it("changes nothing when the user declines", async () => {
    const update = vi.fn(async () => {});
    const initialiser = new ModuleInitialiser({ ask: async () => false, update });

    await initialiser.offer("lib/a");

    expect(update).not.toHaveBeenCalled();
  });

  it("asks once however many clicks arrive while it asks", async () => {
    let yes: (answer: boolean) => void = () => {};
    const ask = vi.fn(() => new Promise<boolean>((resolve) => (yes = resolve)));
    const update = vi.fn(async () => {});
    const initialiser = new ModuleInitialiser({ ask, update });

    const first = initialiser.offer("lib/a");
    await initialiser.offer("lib/a");
    await initialiser.offer("lib/a");
    yes(true);
    await first;

    expect(ask).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalledWith("lib/a");
  });

  it("asks again once the first answer is in", async () => {
    const ask = vi.fn(async () => false);
    const initialiser = new ModuleInitialiser({ ask, update: vi.fn(async () => {}) });

    await initialiser.offer("lib/a");
    await initialiser.offer("lib/a");

    expect(ask).toHaveBeenCalledTimes(2);
  });
});
