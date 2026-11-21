import type { ContextItem, ToolContext, ToolRequest } from "./ipc";
import { SEPARATOR, item, offer, submenu, tidy } from "./context-menu";
import type { PaletteCommand } from "./palette";

/** Preferences ▸ Tools (F-954): the user's own programs in the Tools submenus and the palette. */

export const TOOL_PLACES = ["repository", "file", "commit"] as const;
export type ToolPlace = (typeof TOOL_PLACES)[number];

export const PLACEHOLDERS = ["repo", "file", "relfile", "commit", "shortcommit", "branch"] as const;
export type Placeholder = (typeof PLACEHOLDERS)[number];

export interface UserCommand {
  id: string;
  name: string;
  program: string;
  /** Split into words with "…" and '…' quoting, then each word expanded: no shell. */
  args: string;
  /** Empty is `${repo}`. */
  cwd: string;
  places: ToolPlace[];
  /** Started and left to run (a GUI tool); else its output goes to the Output journal. */
  detached: boolean;
  confirm: boolean;
  /** Re-read the repository when a captured run ends. */
  refresh: boolean;
}

export const MAX_USER_COMMANDS = 50;
export const TOOL_MENU_PREFIX = "tool:";
export const TOOL_PALETTE_PREFIX = "palette-tool:";

/** Words of `line` as the backend splits them (`merge_tool::tokenize`); `null` on an
    unbalanced quote. */
export function splitWords(line: string): string[] | null {
  const words: string[] = [];
  let at = 0;
  while (at < line.length) {
    while (at < line.length && /\s/.test(line[at]!)) at += 1;
    if (at >= line.length) break;
    let word = "";
    while (at < line.length && !/\s/.test(line[at]!)) {
      const c = line[at]!;
      if (c === '"' || c === "'") {
        at += 1;
        let closed = false;
        while (at < line.length) {
          const inner = line[at]!;
          if (inner === c) {
            closed = true;
            at += 1;
            break;
          }
          if (c === '"' && inner === "\\" && line[at + 1] === '"') {
            word += '"';
            at += 2;
            continue;
          }
          word += inner;
          at += 1;
        }
        if (!closed) return null;
        continue;
      }
      word += c;
      at += 1;
    }
    words.push(word);
  }
  return words;
}

/** Every `${…}` the command uses, known or not, each once; an unclosed one as `${…` text. */
export function placeholdersOf(command: Pick<UserCommand, "program" | "args" | "cwd">): string[] {
  const found: string[] = [];
  for (const text of [command.program, command.args, command.cwd]) {
    for (let at = text.indexOf("${"); at >= 0; at = text.indexOf("${", at + 2)) {
      const close = text.indexOf("}", at + 2);
      const name = close < 0 ? text.slice(at) : text.slice(at + 2, close);
      if (!found.includes(name)) found.push(name);
    }
  }
  return found;
}

function known(name: string): name is Placeholder {
  return (PLACEHOLDERS as readonly string[]).includes(name);
}

/** Why the command cannot run anywhere as written, or null. */
export function commandProblem(command: UserCommand): string | null {
  if (command.name.trim() === "") return "Give the command a name.";
  if (command.program.trim() === "") return "Name the program to run.";
  if (splitWords(command.args) === null) return "A quote in Arguments is not closed.";
  const unknown = placeholdersOf(command).find((name) => !known(name));
  if (unknown !== undefined) {
    const shown = unknown.startsWith("${") ? unknown : `\${${unknown}}`;
    return `${shown} is not a placeholder. Use ${PLACEHOLDERS.map((name) => `\${${name}}`).join(", ")}.`;
  }
  return null;
}

/** The placeholders the context has nothing for. `${repo}` always has a value. */
export function missingFor(command: UserCommand, context: ToolContext): Placeholder[] {
  const has: Record<Placeholder, boolean> = {
    repo: true,
    file: context.file !== null,
    relfile: context.file !== null,
    commit: context.commit !== null,
    shortcommit: context.commit !== null,
    branch: context.branch !== null,
  };
  return placeholdersOf(command).filter((name): name is Placeholder => known(name) && !has[name]);
}

const NEEDS: Record<Placeholder, string> = {
  repo: "a repository",
  file: "a selected file",
  relfile: "a selected file",
  commit: "a selected commit",
  shortcommit: "a selected commit",
  branch: "a current branch",
};

/** Why the palette cannot run it here, or null. */
export function toolBlocked(command: UserCommand, context: ToolContext): string | null {
  const missing = missingFor(command, context);
  if (missing.length === 0) return null;
  const needs = [...new Set(missing.map((name) => NEEDS[name]))];
  return `Needs ${needs.join(" and ")}`;
}

function text(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

/** Settings validation: an entry without a name or a program is dropped, ids are made
    unique, unknown places are left out; a hand-edited file cannot break a menu. */
export function knownUserCommands(value: unknown): UserCommand[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const kept: UserCommand[] = [];
  for (const entry of value) {
    if (typeof entry !== "object" || entry === null) continue;
    const record = entry as Record<string, unknown>;
    const name = text(record.name).trim();
    const program = text(record.program).trim();
    if (name === "" || program === "") continue;
    const places = Array.isArray(record.places)
      ? TOOL_PLACES.filter((place) => (record.places as unknown[]).includes(place))
      : [];
    let id = text(record.id).trim() || `tool-${kept.length + 1}`;
    while (kept.some((other) => other.id === id)) id = `${id}-${kept.length + 1}`;
    kept.push({
      id,
      name,
      program,
      args: text(record.args),
      cwd: text(record.cwd),
      places,
      detached: record.detached === true,
      confirm: record.confirm === true,
      refresh: record.refresh === true,
    });
    if (kept.length === MAX_USER_COMMANDS) break;
  }
  return kept;
}

/** A fresh row for the editor: shows output, in every menu, with an id no other row has. */
export function newUserCommand(existing: readonly UserCommand[]): UserCommand {
  let number = existing.length + 1;
  while (existing.some((command) => command.id === `tool-${number}`)) number += 1;
  return {
    id: `tool-${number}`,
    name: "New command",
    program: "",
    args: "",
    cwd: "",
    places: [...TOOL_PLACES],
    detached: false,
    confirm: false,
    refresh: false,
  };
}

/** The Tools submenu of one menu: the commands meant for it whose placeholders the context
    can fill. `null` when none is meant for it; `blocked` turns the row off with a reason. */
export function toolsSubmenu(
  commands: readonly UserCommand[],
  place: ToolPlace,
  context: ToolContext,
  blocked: string | null = null,
): ContextItem | null {
  const meant = commands.filter((command) => command.places.includes(place) && commandProblem(command) === null);
  if (meant.length === 0) return null;
  if (blocked !== null) return offer("tools", "Tools", blocked);
  const shown = meant.filter((command) => missingFor(command, context).length === 0);
  if (shown.length === 0) return null;
  return submenu(
    "tools",
    "Tools",
    shown.map((command) => item(`${TOOL_MENU_PREFIX}${command.id}`, command.name)),
  );
}

/** The menu with the Tools submenu at its end, behind a separator. */
export function withTools(items: readonly ContextItem[], tools: ContextItem | null): ContextItem[] {
  return tools === null ? [...items] : tidy([...items, SEPARATOR, tools]);
}

/** The command id of a Tools menu row, or null for any other row. */
export function toolOf(menuId: string): string | null {
  return menuId.startsWith(TOOL_MENU_PREFIX) ? menuId.slice(TOOL_MENU_PREFIX.length) : null;
}

/** "Tools: <name>" for each command; off with the reason where the context lacks a value. */
export function toolPaletteCommands(
  commands: readonly UserCommand[],
  context: ToolContext | null,
  run: (command: UserCommand) => void,
  blocked: string | null = null,
): PaletteCommand[] {
  return commands
    .filter((command) => commandProblem(command) === null)
    .map((command) => ({
      id: `${TOOL_PALETTE_PREFIX}${command.id}`,
      title: `Tools: ${command.name}`,
      synonyms: ["user command", "external tool", command.program],
      unavailable: blocked ?? (context === null ? "Open a repository first" : (toolBlocked(command, context) ?? undefined)),
      run: () => run(command),
    }));
}

export function toolRequest(command: UserCommand, context: ToolContext): ToolRequest {
  return {
    name: command.name,
    program: command.program,
    args: command.args,
    cwd: command.cwd,
    detached: command.detached,
    context,
  };
}
