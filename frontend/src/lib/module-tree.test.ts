import { describe, expect, it } from "vitest";
import {
  describeModule,
  mayExpand,
  moduleKey,
  moduleRows,
  splitModulePath,
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
