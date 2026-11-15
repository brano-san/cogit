import { describe, expect, it } from "vitest";
import { merge } from "./settings";
import {
  commandProblem,
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

function command(patch: Partial<UserCommand> = {}): UserCommand {
  return {
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
    expect(missingFor(all, NOTHING)).toEqual(["relfile", "shortcommit", "branch"]);
    expect(missingFor(all, { file: "a.txt", commit: "abc", branch: "main" })).toEqual([]);
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
