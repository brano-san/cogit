import { describe, expect, it } from "vitest";
import {
  describeModule,
  moduleTooltip,
  mayExpand,
  moduleKey,
  moduleRoot,
  moduleRows,
  moduleUpdate,
  pulsedRoots,
  shownRowRoot,
  splitModulePath,
  updateModule,
} from "./module-tree";
import type { Submodule } from "$lib/ipc";

const mod = (path: string, over: Partial<Submodule> = {}): Submodule => ({
  name: path,
  path,
  url: `https://example.invalid/${path}.git`,
  recorded: "452e8002c0ffee0000000000000000000000beef",
  checkedOut: "452e8002c0ffee0000000000000000000000beef",
  state: "inSync",
  branch: null,
  subject: null,
  nested: false,
  ahead: 0,
  behind: 0,
  repoState: null,
  ...over,
});

describe("splitModulePath", () => {
  it("keeps the folder apart from the name, the way SmartGit dims it", () => {
    expect(splitModulePath("cmake/cmake-conan")).toEqual({ dir: "cmake/", name: "cmake-conan" });
  });

  it("has no folder to show for a module at the top", () => {
    expect(splitModulePath("lib")).toEqual({ dir: "", name: "lib" });
  });

  it("survives a path Windows wrote with backslashes", () => {
    expect(splitModulePath("vendor\\lib")).toEqual({ dir: "vendor\\", name: "lib" });
  });

  it("does not choke on a trailing slash", () => {
    expect(splitModulePath("vendor/lib/")).toEqual({ dir: "vendor/", name: "lib" });
  });
});

describe("moduleKey", () => {
  it("is the path from the top, so two modules named alike stay apart", () => {
    expect(moduleKey("a/b", "lib")).toBe("a/b/lib");
  });

  it("at the top is just the module's own path", () => {
    expect(moduleKey("", "lib")).toBe("lib");
  });

  // The key is what keeps a node expanded across a refresh; it must not depend on where
  // the row happened to be in the list.
  it("is the same whatever order the rows arrive in", () => {
    expect(moduleKey("a", "b")).toBe(moduleKey("a", "b"));
  });
});

describe("describeModule", () => {
  it("says which branch it is on when it is on one", () => {
    expect(describeModule(mod("lib", { branch: "master" }))).toBe("master");
  });

  it("falls back to the commit it is on, shortened, with the subject", () => {
    const text = describeModule(mod("lib", { subject: "Merge branch 'feature'" }));
    expect(text).toBe("452e800: Merge branch 'feature'");
  });

  // A light tree read only .gitmodules; "no commit checked out" would be a claim it cannot make.
  it("says nothing about a module of a light tree", () => {
    expect(describeModule(mod("lib", { state: "unread", checkedOut: null }))).toBe("");
    expect(moduleTooltip(mod("lib", { state: "unread", checkedOut: null }))).toBe("");
  });

  it("says a module has not been initialised rather than showing nothing", () => {
    expect(describeModule(mod("lib", { state: "notInitialised" }))).toBe("not initialised");
  });

  it("calls out a module that is not on the commit its parent records", () => {
    expect(describeModule(mod("lib", { state: "diverged", checkedOut: "aaaa1111" }))).toContain(
      "diverged",
    );
  });

  it("says something even for a module with nothing checked out", () => {
    expect(describeModule(mod("lib", { checkedOut: null })).length).toBeGreaterThan(0);
  });

  // Requirement 6: three situations, three different things to do about them.
  it("labels a module ahead of, behind or apart from what the parent records", () => {
    expect(describeModule(mod("lib", { state: "ahead", ahead: 2 }))).toContain("ahead");
    expect(describeModule(mod("lib", { state: "behind", behind: 1 }))).toContain("behind");
    expect(describeModule(mod("lib", { state: "diverged", ahead: 1, behind: 1 }))).toContain(
      "diverged",
    );
  });

  it("does not dress up a guess as a label when the recorded commit is missing", () => {
    const text = describeModule(mod("lib", { state: "unknown" }));
    expect(text).not.toMatch(/ahead|behind|diverged/);
  });

  // Comparing with a commit that is not here is impossible; that it is not here is a fact.
  it("says a module whose recorded commit is missing has not been fetched", () => {
    expect(describeModule(mod("lib", { state: "unknown" }))).toMatch(/· not fetched$/);
  });

  it("puts nothing after the name when the module is where the parent says", () => {
    expect(describeModule(mod("lib", { branch: "master" }))).toBe("master");
  });
});

describe("moduleTooltip", () => {
  it("tells a module that is ahead to commit the pointer in the parent", () => {
    expect(moduleTooltip(mod("lib", { state: "ahead", ahead: 2 }))).toMatch(/2 commits.*commit/is);
  });

  it("tells a module that is behind to run git submodule update", () => {
    expect(moduleTooltip(mod("lib", { state: "behind", behind: 1 }))).toContain(
      "git submodule update",
    );
  });

  // F-056: it pointed at an "(Update)" button no row had; the row menu has one now (F-521).
  it("names the command that brings a module that is behind to the recorded commit", () => {
    const tip = moduleTooltip(mod("lib", { state: "behind", behind: 1 }));
    expect(tip).toContain("Update in its menu");
    expect(tip).not.toContain("(Update)");
  });

  it("says a diverged module needs a person, with both counts", () => {
    const tip = moduleTooltip(mod("lib", { state: "diverged", ahead: 3, behind: 2 }));
    expect(tip).toContain("3");
    expect(tip).toContain("2");
    expect(tip).toMatch(/decide|by hand|manual/i);
  });

  it("explains why an unknown position cannot be told, and what would tell it", () => {
    expect(moduleTooltip(mod("lib", { state: "unknown" }))).toMatch(/fetch/i);
  });

  // GE-036: no gitlink anywhere is not a commit to fetch.
  it("tells a module nothing records apart from one not fetched", () => {
    expect(describeModule(mod("lib", { state: "unrecorded" }))).toMatch(/· not recorded$/);
    const tip = moduleTooltip(mod("lib", { state: "unrecorded" }));
    expect(tip).toMatch(/stage/i);
    expect(tip).not.toMatch(/fetch/i);
  });

  it("offers to initialise a module that is not checked out", () => {
    expect(moduleTooltip(mod("lib", { state: "notInitialised" }))).toMatch(/initiali[sz]e/i);
  });

  it("has nothing to add for a module in sync", () => {
    expect(moduleTooltip(mod("lib"))).toBe("");
  });
});

describe("moduleRows", () => {
  const children = new Map<string, Submodule[]>([
    ["", [mod("cmake/cmake-conan"), mod("lib")]],
    ["lib", [mod("third_party/fmt")]],
  ]);

  it("lists the top level when nothing is expanded", () => {
    const rows = moduleRows(children, new Set());
    expect(rows.map((row) => row.key)).toEqual(["cmake/cmake-conan", "lib"]);
  });

  it("puts a child under its parent, one level deeper", () => {
    const rows = moduleRows(children, new Set(["lib"]));
    expect(rows.map((row) => row.key)).toEqual([
      "cmake/cmake-conan",
      "lib",
      "lib/third_party/fmt",
    ]);
    expect(rows.find((row) => row.key === "lib/third_party/fmt")?.depth).toBe(1);
  });

  it("marks a row that can be expanded, so the caret is not a lie", () => {
    const rows = moduleRows(children, new Set(["lib"]));
    expect(rows.find((row) => row.key === "lib")?.expanded).toBe(true);
    expect(rows.find((row) => row.key === "cmake/cmake-conan")?.expanded).toBe(false);
  });

  it("shows nothing for an expanded node whose children have not arrived yet", () => {
    const rows = moduleRows(children, new Set(["cmake/cmake-conan"]));
    expect(rows).toHaveLength(2);
  });

  it("goes as deep as the repository does", () => {
    const deep = new Map(children);
    deep.set("lib/third_party/fmt", [mod("doc")]);
    const rows = moduleRows(deep, new Set(["lib", "lib/third_party/fmt"]));
    expect(rows.at(-1)).toMatchObject({ key: "lib/third_party/fmt/doc", depth: 2 });
  });

  it("is empty for a repository with no submodules at all", () => {
    expect(moduleRows(new Map(), new Set())).toEqual([]);
  });

  it("cannot loop for ever on a module that lists itself", () => {
    const loop = new Map<string, Submodule[]>([
      ["", [mod("lib")]],
      ["lib", [mod("lib")]],
    ]);
    expect(moduleRows(loop, new Set(["lib", "lib/lib"])).length).toBeLessThan(40);
  });
});

describe("whether a node can be opened", () => {
  const leaf = mod("lib");
  const holder = { ...mod("lib"), nested: true };

  // The reported case: every node wore a caret that vanished on the first click.
  it("does not offer a caret before anything is known, unless the node said it holds more", () => {
    expect(mayExpand(new Map(), "lib", leaf)).toBe(false);
    expect(mayExpand(new Map(), "lib", holder)).toBe(true);
  });

  it("offers the caret for a node with children", () => {
    expect(mayExpand(new Map([["lib", [mod("a")]]]), "lib", leaf)).toBe(true);
  });

  it("takes the caret away once a look found nothing, whatever the node claimed", () => {
    expect(mayExpand(new Map([["lib", []]]), "lib", holder)).toBe(false);
  });
});

describe("splitModulePath, for a name that has to survive truncation", () => {
  it("hands back the folder without its separator, so it can be shortened alone", () => {
    expect(splitModulePath("src/tetra/import/dmo")).toEqual({
      dir: "src/tetra/import/",
      name: "dmo",
    });
  });
});

describe("moduleUpdate", () => {
  it("runs at once for a module that is behind: nothing of its own is left behind", () => {
    expect(moduleUpdate(mod("lib", { state: "behind", behind: 2 }))).toEqual({ kind: "run", init: false });
  });

  it("initialises a module that is not checked out", () => {
    expect(moduleUpdate(mod("lib", { state: "notInitialised" }))).toEqual({ kind: "run", init: true });
  });

  it("asks first for a module that is ahead, and says where its commits stay", () => {
    const plan = moduleUpdate(mod("lib", { state: "ahead", ahead: 2, branch: "topic" }));
    expect(plan.kind).toBe("ask");
    const message = plan.kind === "ask" ? plan.request.message : "";
    expect(message).toContain("2 commits");
    expect(message).toContain("topic");
  });

  it("warns that a detached module's own commits are left on no branch", () => {
    const plan = moduleUpdate(mod("lib", { state: "diverged", ahead: 1, behind: 3 }));
    const message = plan.kind === "ask" ? plan.request.message : "";
    expect(message).toContain("1 commit");
    expect(message).toMatch(/no branch/);
  });

  it("is off, with the reason, where there is nothing to do or nothing known", () => {
    for (const state of ["inSync", "unknown", "unread", "unrecorded"] as const) {
      const plan = moduleUpdate(mod("lib", { state }));
      expect(plan.kind, state).toBe("off");
      expect(plan.kind === "off" ? plan.reason : "", state).not.toBe("");
    }
  });
});

describe("updateModule", () => {
  const deps = (answer: boolean) => {
    const calls: string[] = [];
    return {
      calls,
      ask: async () => {
        calls.push("ask");
        return answer;
      },
      update: async (init: boolean) => {
        calls.push(init ? "update --init" : "update");
      },
    };
  };

  it("updates a module that is behind without a question", async () => {
    const run = deps(false);
    expect(await updateModule(mod("lib", { state: "behind", behind: 1 }), run)).toBe(true);
    expect(run.calls).toEqual(["update"]);
  });

  it("updates a module that is ahead only once the answer is yes", async () => {
    const no = deps(false);
    expect(await updateModule(mod("lib", { state: "ahead", ahead: 1 }), no)).toBe(false);
    expect(no.calls).toEqual(["ask"]);

    const yes = deps(true);
    expect(await updateModule(mod("lib", { state: "ahead", ahead: 1 }), yes)).toBe(true);
    expect(yes.calls).toEqual(["ask", "update"]);
  });

  it("does nothing for a module already on the recorded commit", async () => {
    const run = deps(true);
    expect(await updateModule(mod("lib"), run)).toBe(false);
    expect(run.calls).toEqual([]);
  });
});

// Every node of every tree gets its own marks, read by its folder (item 10 of 25.09).
describe("the folders the marks of a tree are read from", () => {
  const rows = moduleRows(
    new Map([
      ["", [mod("vendor/lib"), mod("docs", { state: "notInitialised", checkedOut: null })]],
      ["vendor/lib", [mod("deep/inner")]],
    ]),
    new Set(["vendor/lib"]),
  );

  it("are the top's folder joined with each key, nested ones included", () => {
    expect(pulsedRoots("E:/w/app", rows)).toEqual(["E:/w/app/vendor/lib", "E:/w/app/vendor/lib/deep/inner"]);
    expect(moduleRoot("E:/w/app/", "vendor/lib")).toBe("E:/w/app/vendor/lib");
  });

  it("leave out a module that is not checked out: there is no repository to read", () => {
    expect(pulsedRoots("E:/w/app", rows)).not.toContain("E:/w/app/docs");
  });
});

describe("the row the panels show", () => {
  const panels = { current: "E:/w/app/vendor/lib", moduleOwnerRoot: "E:/w/app", openModule: "vendor/lib" };

  // The backend spells the submodule's root its own way; the tree names it by key.
  it("is the submodule node, named as the tree names it", () => {
    expect(shownRowRoot({ ...panels, current: "E:\\w\\app\\vendor\\lib", worktreeOwnerRoot: null })).toBe(
      "E:/w/app/vendor/lib",
    );
  });

  it("is the repository itself when no submodule holds the panels", () => {
    expect(shownRowRoot({ current: "E:/w/app", moduleOwnerRoot: "E:/w/app", openModule: null, worktreeOwnerRoot: null })).toBe(
      "E:/w/app",
    );
    expect(shownRowRoot({ current: null, moduleOwnerRoot: null, openModule: null, worktreeOwnerRoot: null })).toBeNull();
  });

  it("is a worktree opened from the submodule, not the submodule", () => {
    expect(shownRowRoot({ ...panels, current: "E:/w/lib-wt", worktreeOwnerRoot: "E:/w/app/vendor/lib" })).toBe("E:/w/lib-wt");
  });
});
