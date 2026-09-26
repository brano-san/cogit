import { describe, expect, it } from "vitest";
import {
  ACTIONS,
  DEFAULT_LAYOUT,
  actionOf,
  hintOf,
  NO_FACTS,
  SEPARATOR,
  groupsOf,
  menuOf,
  NO_MENU_CONTEXT,
  type MenuContext,
  type MenuEntry,
  reasonOf,
  refAt,
  shortcutOfAction,
  branchRevision,
  localRevision,
  splitMarked,
  targetsOf,
  type ToolbarFacts,
} from "./toolbar";

const HEAD = "a".repeat(40);
const OTHER = "b".repeat(40);

function facts(over: Partial<ToolbarFacts> = {}): ToolbarFacts {
  return { ...NO_FACTS, repository: true, head: HEAD, ...over };
}

function tree(over: Partial<ToolbarFacts> = {}): ToolbarFacts {
  return facts({ onWorkingTree: true, ...over });
}

describe("splitMarked", () => {
  it("sorts the selection into the list each path is in", () => {
    const split = splitMarked({
      marked: ["a.txt", "b.txt", "both.txt"],
      unstaged: [{ path: "a.txt" }, { path: "both.txt" }],
      staged: [{ path: "b.txt" }, { path: "both.txt" }],
    });
    expect(split.markedUnstaged).toEqual(["a.txt", "both.txt"]);
    expect(split.markedStaged).toEqual(["b.txt", "both.txt"]);
  });

  it("drops a path that has left both lists since it was selected", () => {
    const split = splitMarked({ marked: ["gone.txt"], unstaged: [], staged: [] });
    expect(split).toEqual({ markedUnstaged: [], markedStaged: [] });
  });
});

describe("Stage", () => {
  it("needs the Working Tree, not a commit from history", () => {
    expect(reasonOf("stage", facts({ unstaged: ["a"], commit: OTHER }))).toMatch(/Working Tree/);
  });

  it("runs on selected unstaged files", () => {
    expect(reasonOf("stage", tree({ unstaged: ["a"], markedUnstaged: ["a"] }))).toBeUndefined();
  });

  it("runs on everything when nothing is selected and there is a change", () => {
    expect(reasonOf("stage", tree({ unstaged: ["a"] }))).toBeUndefined();
  });

  it("stays off when the selection holds only staged files", () => {
    expect(reasonOf("stage", tree({ unstaged: ["a"], staged: ["b"], markedStaged: ["b"] }))).toBe(
      "The selected files have nothing to stage",
    );
  });

  it("stays off on a clean tree", () => {
    expect(reasonOf("stage", tree())).toBe("Nothing to stage");
  });
});

describe("Unstage", () => {
  it("runs on selected staged files", () => {
    expect(reasonOf("unstage", tree({ staged: ["a"], markedStaged: ["a"] }))).toBeUndefined();
  });

  it("runs on the whole index when nothing is selected", () => {
    expect(reasonOf("unstage", tree({ staged: ["a"] }))).toBeUndefined();
  });

  it("stays off when the selected files are not staged", () => {
    expect(
      reasonOf("unstage", tree({ staged: ["b"], unstaged: ["a"], markedUnstaged: ["a"] })),
    ).toBe("None of the selected files is staged");
  });

  it("stays off with an empty index", () => {
    expect(reasonOf("unstage", tree({ unstaged: ["a"] }))).toBe("Nothing is staged");
  });
});

describe("Discard", () => {
  it("needs selected working-tree changes", () => {
    expect(reasonOf("discard", tree({ unstaged: ["a"], markedUnstaged: ["a"] }))).toBeUndefined();
  });

  it("does not throw away everything when nothing is selected", () => {
    expect(reasonOf("discard", tree({ unstaged: ["a"] }))).toMatch(/Select the changes/);
  });

  it("has nothing to discard in a file that is only staged", () => {
    expect(reasonOf("discard", tree({ staged: ["a"], markedStaged: ["a"] }))).toMatch(
      /Select the changes/,
    );
  });
});

describe("Merge", () => {
  it("needs a selected commit", () => {
    expect(reasonOf("merge", facts())).toBe("Select a commit or a branch first");
  });

  it("is off for HEAD itself", () => {
    expect(reasonOf("merge", facts({ commit: HEAD, merged: true }))).toBe("HEAD itself is selected");
  });

  it("is off for an ancestor of HEAD", () => {
    expect(reasonOf("merge", facts({ commit: OTHER, merged: true }))).toBe(
      "HEAD already contains the selected commit",
    );
  });

  it("waits while the ancestry is being asked", () => {
    expect(reasonOf("merge", facts({ commit: OTHER }))).toMatch(/Checking/);
  });

  it("runs for a commit HEAD does not contain", () => {
    expect(reasonOf("merge", facts({ commit: OTHER, merged: false }))).toBeUndefined();
  });
});

describe("Rebase", () => {
  it("runs for any commit other than HEAD", () => {
    expect(reasonOf("rebase", facts({ commit: OTHER, merged: true }))).toBeUndefined();
    expect(reasonOf("rebase-i", facts({ commit: OTHER }))).toBeUndefined();
  });

  it("is off for HEAD and without a selection", () => {
    expect(reasonOf("rebase", facts({ commit: HEAD }))).toBe("HEAD itself is selected");
    expect(reasonOf("rebase", facts())).toBe("Select a commit or a branch first");
  });
});

describe("the rest", () => {
  it("blames the missing repository before anything else", () => {
    for (const id of ["pull", "stage", "unstage", "discard", "merge", "rebase", "stash", "undo"]) {
      expect(reasonOf(id, NO_FACTS), id).toBe("No repository is open");
    }
  });

  it("needs a remote for the network actions", () => {
    expect(reasonOf("pull", facts())).toBe("This repository has no remote");
    expect(reasonOf("push", facts({ remote: true, branch: true }))).toBeUndefined();
  });

  // In a detached HEAD, mid-rebase or on a branch that tracks nothing every click on Pull,
  // Push or Sync ended in a git error: "You are not currently on a branch".
  it("pulls and syncs only a branch that tracks a remote branch", () => {
    for (const id of ["pull", "sync"]) {
      expect(reasonOf(id, facts({ remote: true })), id).toBe("HEAD is not on a branch");
      expect(reasonOf(id, facts({ remote: true, branch: true })), id).toBe("The branch tracks no remote branch");
      expect(reasonOf(id, facts({ remote: true, branch: true, upstream: true })), id).toBeUndefined();
    }
  });

  // A branch without upstream is pushed with --set-upstream (R-414); only a detached HEAD
  // has nothing to push.
  it("pushes a branch whether or not it tracks one yet", () => {
    expect(reasonOf("push", facts({ remote: true }))).toBe("HEAD is not on a branch");
    expect(reasonOf("push", facts({ remote: true, branch: true }))).toBeUndefined();
  });

  it("never offers an action it does not know", () => {
    expect(reasonOf("teleport", facts())).toBe("Not built yet");
  });
});

describe("targetsOf", () => {
  it("stages the selection when there is one", () => {
    expect(targetsOf("stage", tree({ unstaged: ["a", "b"], markedUnstaged: ["b"] }))).toEqual([
      "b",
    ]);
  });

  it("stages everything unstaged when nothing is selected", () => {
    expect(targetsOf("stage", tree({ unstaged: ["a", "b"] }))).toEqual(["a", "b"]);
  });

  it("unstages the whole index when nothing is selected", () => {
    expect(targetsOf("unstage", tree({ staged: ["c"] }))).toEqual(["c"]);
  });

  it("discards only what is selected", () => {
    expect(targetsOf("discard", tree({ unstaged: ["a", "b"], markedUnstaged: ["a"] }))).toEqual([
      "a",
    ]);
  });
});

describe("refAt", () => {
  const branch = (name: string, kind: "local" | "remote", oid: string, isHead = false) => ({
    name,
    fullName: kind === "local" ? `refs/heads/${name}` : `refs/remotes/${name}`,
    kind,
    oid,
    isHead,
  });
  const branches = [branch("main", "local", HEAD, true), branch("origin/topic", "remote", OTHER), branch("topic", "local", OTHER)];

  it("names the local branch at the commit", () => {
    expect(refAt(OTHER, branches, [])).toBe("topic");
  });

  it("falls back to the hash when no branch points there", () => {
    expect(refAt("c".repeat(40), branches, [])).toBe("c".repeat(40));
  });

  it("skips HEAD's own branch", () => {
    expect(refAt(HEAD, branches, [])).toBe(HEAD);
  });

  // Git reads refs/tags/<name> before refs/heads/<name>: with a tag `topic` on an older
  // commit, Merge "topic" merged the tag and said only "Already up to date".
  it("names the branch in full when a tag of that name would be read first", () => {
    expect(refAt(OTHER, branches, [{ name: "topic" }])).toBe("refs/heads/topic");
  });

  it("names a remote branch in full when a local branch or tag shares its name", () => {
    const remoteOnly = [branch("origin/topic", "remote", OTHER), branch("origin/topic", "local", "c".repeat(40))];
    expect(refAt(OTHER, remoteOnly, [])).toBe("refs/remotes/origin/topic");
    expect(refAt(OTHER, [branch("origin/topic", "remote", OTHER)], [{ name: "origin/topic" }])).toBe(
      "refs/remotes/origin/topic",
    );
    expect(refAt(OTHER, [branch("origin/topic", "remote", OTHER)], [])).toBe("origin/topic");
  });
});

describe("localRevision", () => {
  // Dragging topic onto main in Branches merged a tag topic just the same.
  it("names a dragged local branch in full when a tag of that name would be read first", () => {
    const topic = { name: "topic", fullName: "refs/heads/topic", kind: "local" as const, oid: OTHER, isHead: false };
    expect(localRevision("topic", [topic], [{ name: "topic" }])).toBe("refs/heads/topic");
    expect(localRevision("topic", [topic], [])).toBe("topic");
    expect(localRevision("gone", [topic], [])).toBe("gone");
  });
});

describe("branchRevision", () => {
  it("keeps the short name git writes into the merge message while it is unambiguous", () => {
    const topic = { name: "topic", fullName: "refs/heads/topic", kind: "local" as const };
    expect(branchRevision(topic, [], [])).toBe("topic");
    expect(branchRevision(topic, [], [{ name: "topic" }])).toBe("refs/heads/topic");
  });
});

describe("groupsOf", () => {
  it("splits the default layout into its four groups", () => {
    const groups = groupsOf(DEFAULT_LAYOUT).map((group) => group.map((action) => action.id));
    expect(groups).toEqual([
      ["pull", "push", "sync"],
      ["stage", "unstage", "discard"],
      ["stash", "apply-stash", "merge", "rebase", "tag"],
      ["undo"],
    ]);
  });

  it("drops unknown ids and the groups they leave empty", () => {
    const groups = groupsOf(["teleport", SEPARATOR, SEPARATOR, "pull", SEPARATOR]);
    expect(groups.map((group) => group.map((action) => action.id))).toEqual([["pull"]]);
  });
});

describe("every button", () => {
  it("has a rule, so none of them can be on while its state says off", () => {
    for (const action of ACTIONS) {
      expect(reasonOf(action.id, NO_FACTS), action.id).not.toBe("Not built yet");
    }
  });

  it("has a menu exactly when it is a split button", () => {
    for (const action of ACTIONS) {
      expect(menuOf(action.id).length > 0, action.id).toBe(Boolean(action.split));
    }
  });
});

describe("the Pull menu", () => {
  const menu = (over: Partial<MenuContext> = {}) =>
    menuOf("pull", { ...NO_MENU_CONTEXT, remotes: ["origin", "custom-controls", "beta"], current: "origin", ...over });
  const labels = (entries: MenuEntry[]) =>
    entries.map((entry) => (entry.kind === "separator" ? "—" : entry.label));

  it("lists Pull, the fetches with the current remote first, Fetch All, then the options", () => {
    expect(labels(menu())).toEqual([
      "Pull",
      "—",
      "Fetch 'origin' (current)",
      "Fetch 'beta'",
      "Fetch 'custom-controls'",
      "Fetch All",
      "—",
      "Pull Uses the Current Remote",
      "Pull Uses All Remotes",
      "—",
      "Delete Merged Branches after Pull",
    ]);
  });

  it("builds one fetch per remote of the repository", () => {
    const ids = menu({ remotes: ["upstream"], current: "upstream" })
      .filter((entry) => entry.kind === "item")
      .map((entry) => entry.id);
    expect(ids).toEqual(["pull", "fetch-remote:upstream", "fetch-remotes"]);
  });

  it("ticks the remembered choices", () => {
    const checked = menu({
      prefs: { ...NO_MENU_CONTEXT.prefs, pullScope: "all", deleteMergedAfterPull: true },
    })
      .flatMap((entry) => (entry.kind === "radio" || entry.kind === "check" ? [entry] : []))
      .filter((entry) => entry.checked)
      .map((entry) => entry.id);
    expect(checked).toEqual(["pull-scope:all", "delete-merged"]);
  });

  it("offers a per-remote fetch under the fetch rule", () => {
    expect(reasonOf("fetch-remote:origin", facts({ remote: true }))).toBeUndefined();
    expect(reasonOf("fetch-remote:origin", facts())).toBe("This repository has no remote");
  });
});

describe("the Sync menu", () => {
  const sync = actionOf("sync")!;
  const withOrder = (syncOrder: "pullThenPush" | "pushThenPull"): MenuContext => ({
    ...NO_MENU_CONTEXT,
    prefs: { ...NO_MENU_CONTEXT.prefs, syncOrder },
  });

  it("offers both orders, the remembered one ticked", () => {
    expect(menuOf("sync", withOrder("pullThenPush"))).toEqual([
      expect.objectContaining({ id: "sync-order:pushThenPull", label: "Push, then Pull", checked: false }),
      expect.objectContaining({ id: "sync-order:pullThenPush", label: "Pull, then Push", checked: true }),
    ]);
  });

  it("says in the tooltip what the button will do", () => {
    expect(hintOf(sync, withOrder("pullThenPush"))).toBe("Pull, then push");
    expect(hintOf(sync, withOrder("pushThenPull"))).toBe("Push, then pull");
  });

  it("needs a remote like the rest of Sync", () => {
    expect(reasonOf("sync-order:pushThenPull", facts())).toBe("This repository has no remote");
  });
});

describe("the Stash menu", () => {
  it("offers Stash Selection and the two quick stashes; the button itself is Stash All", () => {
    expect(menuOf("stash").map((entry) => (entry.kind === "separator" ? "—" : entry.label))).toEqual([
      "Stash Selection",
      "Quick Stash All",
      "Quick Stash Selection",
    ]);
  });

  it("turns the selection entries off without a file selected in Files", () => {
    const none = tree({ unstaged: ["a"] });
    expect(reasonOf("stash-selection", none)).toBe("No file is selected in Files");
    expect(reasonOf("quick-stash-selection", none)).toBe("No file is selected in Files");
    expect(reasonOf("quick-stash-all", none)).toBeUndefined();
  });

  it("stashes a selected file once even when it is in both lists", () => {
    const both = tree({ unstaged: ["a", "b"], staged: ["a"], markedUnstaged: ["a"], markedStaged: ["a"] });
    expect(targetsOf("stash-selection", both)).toEqual(["a"]);
    expect(reasonOf("stash-selection", both)).toBeUndefined();
  });

  it("has nothing to stash on a clean tree", () => {
    expect(reasonOf("stash", tree())).toBe("The working tree is clean");
    expect(reasonOf("quick-stash-all", tree())).toBe("The working tree is clean");
  });
});

describe("the Push menu", () => {
  it("offers Push and Push To…", () => {
    expect(menuOf("push").map((entry) => (entry.kind === "separator" ? "—" : entry.id))).toEqual([
      "push",
      "push-to",
    ]);
  });

  it("needs HEAD on a branch for Push To…", () => {
    expect(reasonOf("push-to", facts({ remote: true }))).toBe("HEAD is not on a branch");
    expect(reasonOf("push-to", facts({ remote: true, branch: true }))).toBeUndefined();
  });
});

describe("Apply Stash", () => {
  it("sits beside Stash", () => {
    const at = DEFAULT_LAYOUT.indexOf("stash");
    expect(DEFAULT_LAYOUT[at + 1]).toBe("apply-stash");
  });

  it("is off without a stash and on with one", () => {
    expect(reasonOf("apply-stash", facts())).toBe("There are no stashes");
    expect(reasonOf("apply-stash", facts({ stashes: 2 }))).toBeUndefined();
  });
});

// The Stash button said Ctrl+S after Stash All moved to another key.
describe("shortcutOfAction", () => {
  const keys = { stash: "CmdOrCtrl+Shift+H", pull: "CmdOrCtrl+Shift+U", synchronize: "" };
  const action = (id: string) => ACTIONS.find((entry) => entry.id === id)!;

  it("shows the key of the button's command as the keymap has it", () => {
    expect(shortcutOfAction(action("stash"), keys, false)).toBe("Ctrl+Shift+H");
    expect(shortcutOfAction(action("pull"), keys, true)).toBe("Cmd+Shift+U");
  });

  it("shows no key for a command whose key was removed", () => {
    expect(shortcutOfAction(action("sync"), keys, false)).toBeUndefined();
  });

  it("shows the page's own key for Discard, which no menu item has", () => {
    expect(shortcutOfAction(action("discard"), keys, false)).toBe("Ctrl+Z");
    expect(shortcutOfAction(action("discard"), keys, true)).toBe("Cmd+Z");
  });

  it("shows nothing for a button without a key", () => {
    expect(shortcutOfAction(action("merge"), keys, false)).toBeUndefined();
  });
});
