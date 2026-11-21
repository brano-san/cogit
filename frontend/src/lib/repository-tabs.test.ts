import { describe, expect, it } from "vitest";
import { cycleTab, moveTab, neighborAfterClose, reconcileTabs, tabsToClose } from "./repository-tabs";
import { readSession, withActive, withOpened, writeSession } from "./session";
import { merge } from "./settings";

describe("repository tabs", () => {
  const tabs = ["a", "c", "b"];

  it("preserves restored entries and appends newly opened repositories once", () => {
    expect(reconcileTabs(tabs, ["a", "d", "d"], ["b", "c"])).toEqual(["a", "c", "b", "d"]);
    expect(reconcileTabs(tabs, ["b", "a"], [])).toEqual(["a", "b"]);
  });

  it("activates without changing tab order", () => {
    expect(withActive(readSession({ repositories: tabs }), "c").repositories).toEqual(tabs);
  });

  it("registers a new root at first successful open even before the next list refresh", () => {
    const open = withOpened(readSession({ repositories: tabs }), "d");
    expect(open.repositories).toEqual([...tabs, "d"]);
    expect(withOpened(open, "a").repositories).toEqual([...tabs, "d"]);
  });

  it("restores the dragged order from the session", () => {
    const session = readSession({ repositories: moveTab(tabs, "a", "b") });
    expect(readSession(JSON.parse(writeSession(session))).repositories).toEqual(["c", "b", "a"]);
    expect(moveTab(tabs, "b", "a")).toEqual(["b", "a", "c"]);
  });

  it("selects the right neighbor, or the left one at the end", () => {
    expect(neighborAfterClose(tabs, "a")).toBe("c");
    expect(neighborAfterClose(tabs, "c")).toBe("b");
    expect(neighborAfterClose(tabs, "b")).toBe("c");
    expect(neighborAfterClose(["a"], "a")).toBeNull();
  });

  it("closes exactly the tabs addressed by a bulk command", () => {
    expect(tabsToClose(tabs, "c", "others")).toEqual(["a", "b"]);
    expect(tabsToClose(tabs, "c", "right")).toEqual(["b"]);
    expect(tabsToClose(tabs, "b", "right")).toEqual([]);
    expect(tabsToClose(tabs, "gone", "others")).toEqual([]);
  });

  it("cycles in visible order, wraps, and starts at the ends after restore", () => {
    expect(cycleTab(tabs, "b", 1)).toBe("a");
    expect(cycleTab(tabs, "a", -1)).toBe("b");
    expect(cycleTab(tabs, null, 1)).toBe("a");
    expect(cycleTab(tabs, null, -1)).toBe("b");
    expect(cycleTab([], null, 1)).toBeNull();
  });

  it("shows tabs by default and honors a saved hidden bar", () => {
    expect(merge({}).showRepositoryTabs).toBe(true);
    expect(merge({ showRepositoryTabs: false }).showRepositoryTabs).toBe(false);
  });
});
