import { describe, expect, it } from "vitest";
import { merge } from "./settings";
import {
  commandProblem,
  commandMatchesFiles,
  mergeToolCommands,
  toolShortcutProblem,
  toolShortcuts,
  repositoryToolShortcuts,
  commitToolOutput,
  knownUserCommands,
  MAX_USER_COMMANDS,
  missingFor,
  newUserCommand,
  placeholdersOf,
  splitWords,
  toolOf,
  toolPaletteCommands,
  toolsSubmenu,
  withTools,
  type UserCommand,
} from "./user-commands";
import { item } from "./context-menu";
import { pageCommand } from "./page-accelerators";

function command(patch: Partial<UserCommand> = {}): UserCommand {
  return {
    ...newUserCommand([]),
    id: "t",
    name: "Tool",
    program: "tool.exe",
    args: "",
    cwd: "",
    places: ["repository", "file", "commit"],
    detached: false,
    confirm: false,
    refresh: false,
    ...patch,
  };
}

const NOTHING = { file: null, commit: null, branch: null };

describe("splitWords", () => {
  it("splits at whitespace and keeps quoted spaces in one word", () => {
    expect(splitWords(`log  --format="%h %s" '${"${relfile}"} x' a""b ""`)).toEqual([
      "log",
      "--format=%h %s",
      "${relfile} x",
      "ab",
      "",
    ]);
  });

  it("reads \\\" inside double quotes as a quote", () => {
    expect(splitWords(String.raw`"say \"hi\""`)).toEqual(['say "hi"']);
  });

  it("refuses an unclosed quote", () => {
    expect(splitWords(`"open`)).toBeNull();
    expect(splitWords(`'open`)).toBeNull();
  });
});

describe("placeholders", () => {
  it("lists every placeholder of the program, the arguments and the folder once", () => {
    expect(
      placeholdersOf({ program: "${repo}/x", args: "${file} ${commit} ${file}", cwd: "${branch}" }),
    ).toEqual(["repo", "file", "commit", "branch"]);
  });

  it("names an unknown or unclosed placeholder as a problem; environment variables are text", () => {
    expect(commandProblem(command({ args: "$HOME %PATH%" }))).toBeNull();
    expect(commandProblem(command({ args: "${home}" }))).toMatch(/^\$\{home\} is not a placeholder/);
    expect(commandProblem(command({ args: "${file" }))).toMatch(/^\$\{file is not a placeholder/);
    expect(commandProblem(command({ args: '"x' }))).toBe("A quote in Arguments is not closed.");
    expect(commandProblem(command({ name: " " }))).toBe("Give the command a name.");
    expect(commandProblem(command({ program: "" }))).toBe("Name the program to run.");
  });

  it("knows which placeholders a context cannot fill", () => {
    const all = command({ args: "${repo} ${relfile} ${shortcommit} ${branch}" });
    expect(missingFor(all, NOTHING)).toEqual(["relfile", "shortcommit"]);
    expect(missingFor(all, { file: "a.txt", commit: "abc", branch: "main" })).toEqual([]);
  });
});

describe("extended commands", () => {
  it("matches every selected path against basename or recursive path patterns with desktop case rules", () => {
    const selected = ["icons/Logo.PNG", "docs/guide.md", "docs/a/b.md"];
    const tool = command({ patterns: "*.png; docs/**/*.md" });
    expect(commandMatchesFiles(tool, selected, "windows")).toBe(true);
    expect(commandMatchesFiles(tool, selected, "mac")).toBe(true);
    expect(commandMatchesFiles(tool, selected, "linux")).toBe(false);
    expect(commandMatchesFiles(tool, [...selected, "other/note.md"], "mac")).toBe(false);
    expect(commandMatchesFiles(command(), selected, "linux")).toBe(true);
    expect(toolsSubmenu([tool], "file", { ...NOTHING, file: "x.txt", files: ["x.txt"] })).toBeNull();
  });

  it("suffixes imported names and keeps distinct command IDs", () => {
    const original = [command(), command({ name: "Tool (2)", id: "other" })];
    const merged = mergeToolCommands(original, [command(), command({ id: "next", name: "tool" })]);
    expect(merged.map((tool) => tool.name)).toEqual(["Tool", "Tool (2)", "Tool (3)", "tool (4)"]);
    expect(new Set(merged.map((tool) => tool.id)).size).toBe(4);
  });

  it("guards built-in and user shortcut conflicts and protects native Mac shortcuts", () => {
    const one = command({ shortcut: "CmdOrCtrl+Alt+U" });
    expect(toolShortcutProblem(one, [one], { first: "Alt+Ctrl+u" }, false)).toContain("first");
    expect(toolShortcutProblem(one, [one, command({ id: "second", name: "Other", shortcut: one.shortcut })], {}, false)).toContain("Other");
    expect(toolShortcutProblem(command({ shortcut: "Cmd+Q" }), [], {}, true)).toContain("macOS");
    expect(toolShortcutProblem(one, [one], {}, false)).toBeNull();
  });

  it("gives menu and palette the recorded key and resolves it through page dispatch", () => {
    const one = command({ shortcut: "CmdOrCtrl+Alt+U" });
    const keys = toolShortcuts([one], {});
    const [row] = toolPaletteCommands([one], NOTHING, () => {}, null, keys);
    expect(row?.shortcut).toBe("Ctrl+Alt+U");
    expect(toolsSubmenu([one], "repository", NOTHING, null, keys)?.children?.[0]?.accelerator).toBe(one.shortcut);
    const binding = { id: row!.id, label: one.name, section: "Tools", defaultAccelerator: null, onBar: false };
    const press = { key: "u", code: "KeyU", ctrlKey: true, altKey: true, metaKey: false, shiftKey: false };
    expect(pageCommand(press, [binding], keys, { windows: false, mac: false })).toBe(row?.id);
    expect(toolShortcuts([one], { operation: one.shortcut })).toEqual({});
  });

  it("shows repository commands separately and marks untrusted commands for review", () => {
    const shared = command({ name: "Repository lint", source: { hash: "digest", index: 0 }, shortcut: "CmdOrCtrl+Alt+U" });
    const menu = toolsSubmenu([shared], "repository", NOTHING, null, {}, true, false);
    expect(menu).toMatchObject({ label: "Repository commands", children: [{ label: "Repository lint (review trust…)" }] });
    expect(toolPaletteCommands([shared], NOTHING, () => {}, null, {}, false)[0]?.title)
      .toBe("Repository commands: Repository lint (review trust…)");
    expect(toolsSubmenu([shared], "repository", NOTHING, "SSH repository", {}, true, false)?.children?.[0]?.enabled).toBe(false);
    const own = command({ shortcut: shared.shortcut });
    expect(repositoryToolShortcuts([shared], [own], {})).toEqual({});
    expect(toolPaletteCommands([shared], NOTHING, () => {}, null, {}, true)[0]?.title).toContain("shortcut unavailable");
    expect(toolPaletteCommands([shared], NOTHING, () => {}, null, repositoryToolShortcuts([shared], [], {}), true)[0]?.shortcut)
      .toBe("Ctrl+Alt+U");
  });

  it("appends output by default and preserves a draft when replacement is declined", () => {
    expect(commitToolOutput("Draft", "New\r\ntext\n", false, false)).toBe("Draft\nNew\ntext");
    expect(commitToolOutput("Draft", "New\n", true, false)).toBe("Draft");
    expect(commitToolOutput("Draft", "New\n", true, true)).toBe("New");
    expect(commitToolOutput("", "New\n", true, false)).toBe("New");
  });
});

describe("Tools submenu", () => {
  const repoOnly = command({ id: "repo", name: "Repo", args: "${repo}" });
  const fileOnly = command({ id: "file", name: "File", args: "${file}" });
  const commitOnly = command({ id: "commit", name: "Commit", args: "${commit}", places: ["commit"] });
  const broken = command({ id: "broken", name: "Broken", args: "${nope}" });
  const all = [repoOnly, fileOnly, commitOnly, broken];

  const ids = (menu: ReturnType<typeof toolsSubmenu>) => menu?.children?.map((row) => row.id);

  it("shows in a menu only what was ticked for it and what its context can fill", () => {
    expect(ids(toolsSubmenu(all, "repository", { ...NOTHING, branch: "main" }))).toEqual(["tool:repo"]);
    expect(ids(toolsSubmenu(all, "file", { ...NOTHING, file: "a b.txt" }))).toEqual(["tool:repo", "tool:file"]);
    expect(ids(toolsSubmenu(all, "commit", { ...NOTHING, commit: "abc" }))).toEqual(["tool:repo", "tool:commit"]);
  });

  it("is left out when nothing is meant for that menu or nothing fits", () => {
    expect(toolsSubmenu([commitOnly], "repository", NOTHING)).toBeNull();
    expect(toolsSubmenu([fileOnly], "repository", NOTHING)).toBeNull();
    expect(toolsSubmenu([], "file", NOTHING)).toBeNull();
  });

  it("is off with a reason on a closed repository", () => {
    expect(toolsSubmenu([repoOnly], "repository", NOTHING, "closed")).toMatchObject({
      label: "Tools (closed)",
      enabled: false,
    });
  });

  it("goes at the end of the menu behind a separator", () => {
    const menu = withTools([item("a", "A")], toolsSubmenu([repoOnly], "repository", NOTHING));
    expect(menu.map((row) => row.id)).toEqual(["a", "", "tools"]);
    expect(withTools([item("a", "A")], null).map((row) => row.id)).toEqual(["a"]);
    expect(toolOf("tool:repo")).toBe("repo");
    expect(toolOf("repo-open")).toBeNull();
  });

  it("puts each command in the palette, off with the reason where the context lacks a value", () => {
    const palette = toolPaletteCommands(all, { ...NOTHING, commit: "abc" }, () => {});
    expect(palette.map((entry) => [entry.title, entry.unavailable])).toEqual([
      ["Tools: Repo", undefined],
      ["Tools: File", "Needs a selected file"],
      ["Tools: Commit", undefined],
    ]);
    expect(toolPaletteCommands([repoOnly], null, () => {})[0]?.unavailable).toBe("Open a repository first");
  });
});

describe("settings", () => {
  it("keeps valid commands, drops entries without a name or a program, makes ids unique", () => {
    const kept = knownUserCommands([
      { id: "a", name: " Lint ", program: "npm", args: "run lint", places: ["file", "elsewhere"], refresh: true },
      { id: "a", name: "Gitk", program: "gitk", detached: true },
      { name: "", program: "x" },
      { name: "No program" },
      "text",
      null,
    ]);
    expect(kept).toEqual([
      {
        ...newUserCommand([]),
        id: "a",
        name: "Lint",
        program: "npm",
        args: "run lint",
        cwd: "",
        places: ["file"],
        detached: false,
        confirm: false,
        refresh: true,
      },
      {
        ...newUserCommand([]),
        id: "a-2",
        name: "Gitk",
        program: "gitk",
        args: "",
        cwd: "",
        places: [],
        detached: true,
        confirm: false,
        refresh: false,
      },
    ]);
    expect(knownUserCommands("x")).toBeUndefined();
    const many = Array.from({ length: MAX_USER_COMMANDS + 5 }, (_, at) => ({ id: `${at}`, name: "n", program: "p" }));
    expect(knownUserCommands(many)).toHaveLength(MAX_USER_COMMANDS);
  });

  it("reads them through merge, and a damaged value falls back to none", () => {
    expect(merge({ userCommands: [{ id: "x", name: "X", program: "x" }] } as never).userCommands).toHaveLength(1);
    expect(merge({ userCommands: "nope" } as never).userCommands).toEqual([]);
    expect(merge(null).userCommands).toEqual([]);
  });

  it("gives a new row an id no other row has", () => {
    const first = newUserCommand([]);
    const second = newUserCommand([first, command({ id: "tool-2" })]);
    expect(new Set([first.id, second.id, "tool-2"]).size).toBe(3);
    expect(first.places).toEqual(["repository", "file", "commit"]);
  });
});
