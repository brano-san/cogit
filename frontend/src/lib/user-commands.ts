import type { ContextItem, ToolContext, ToolRequest } from "./ipc";
import { SEPARATOR, offer, submenu, tidy } from "./context-menu";
import type { PaletteCommand } from "./palette";
import { fileName } from "./files";
import { canonicalShortcut, claimable, prettyKeys, shortcutRefusal, type Keymap } from "./keymap";
import { ON_MAC, OS, type Os } from "./platform";
import type { RepositoryTool } from "./ipc/bindings";

/** Preferences ▸ Tools (F-954): the user's own programs in the Tools submenus and the palette. */

export const TOOL_PLACES = ["repository", "file", "commit"] as const;
export type ToolPlace = (typeof TOOL_PLACES)[number];

export const PLACEHOLDERS = ["repo", "root", "file", "relfile", "files", "commit", "shortcommit", "branch", "upstream", "remote", "issue", "line"] as const;
export type Placeholder = (typeof PLACEHOLDERS)[number];

export const OUTPUT_MODES = ["panel", "notification", "message", "silent"] as const;
export type ToolOutput = (typeof OUTPUT_MODES)[number];

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
  patterns: string;
  shortcut: string;
  output: ToolOutput;
  replaceMessage: boolean;
  source?: RepositoryTool;
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
  const words = splitWords(command.args);
  if (words === null) return "A quote in Arguments is not closed.";
  const files = "${files}";
  if (command.program.includes(files) || command.cwd.includes(files) || words.some((word) => word.includes(files) && word !== files)) {
    return "${files} must be a separate argument.";
  }
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
    root: true,
    file: context.file !== null,
    relfile: context.file !== null,
    files: selectedToolFiles(context).length > 0,
    commit: context.commit !== null,
    shortcommit: context.commit !== null,
    branch: true,
    upstream: true,
    remote: true,
    issue: true,
    line: (context.line ?? 0) > 0,
  };
  return placeholdersOf(command).filter((name): name is Placeholder => known(name) && !has[name]);
}

const NEEDS: Record<Placeholder, string> = {
  repo: "a repository",
  root: "a repository",
  file: "a selected file",
  relfile: "a selected file",
  files: "a selected file",
  commit: "a selected commit",
  shortcommit: "a selected commit",
  branch: "a current branch",
  upstream: "an upstream branch",
  remote: "a remote",
  issue: "an issue ID",
  line: "a known line number",
};

/** Why the palette cannot run it here, or null. */
export function toolBlocked(command: UserCommand, context: ToolContext): string | null {
  if (command.places.includes("file") && !commandMatchesFiles(command, selectedToolFiles(context))) {
    return "Needs selected files that match Applies to";
  }
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
      patterns: text(record.patterns).trim(),
      shortcut: text(record.shortcut).trim(),
      output: OUTPUT_MODES.find((mode) => mode === record.output) ?? "panel",
      replaceMessage: record.replaceMessage === true,
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
    patterns: "",
    shortcut: "",
    output: "panel",
    replaceMessage: false,
  };
}

/** The Tools submenu of one menu: the commands meant for it whose placeholders the context
    can fill. `null` when none is meant for it; `blocked` turns the row off with a reason. */
export function toolsSubmenu(
  commands: readonly UserCommand[],
  place: ToolPlace,
  context: ToolContext,
  blocked: string | null = null,
  shortcuts: Keymap = {},
  repository = false,
  trusted = true,
): ContextItem | null {
  const meant = commands.filter((command) => command.places.includes(place) && commandProblem(command) === null);
  if (meant.length === 0) return null;
  const id = repository ? "repository-tools" : "tools";
  const label = repository ? "Repository commands" : "Tools";
  const matching = place === "file" ? meant.filter((command) => commandMatchesFiles(command, selectedToolFiles(context))) : meant;
  if (matching.length === 0) return null;
  if (blocked !== null && !repository) return offer(id, label, blocked);
  const shown = blocked === null ? matching.filter((command) => missingFor(command, context).length === 0) : matching;
  if (shown.length === 0) return null;
  return submenu(
    id,
    label,
    shown.map((command) => offer(toolMenuId(command), toolLabel(command, trusted, shortcuts), blocked,
      shortcuts[toolPaletteId(command)])),
  );
}

/** The menu with the Tools submenu at its end, behind a separator. */
export function withTools(items: readonly ContextItem[], ...tools: (ContextItem | null)[]): ContextItem[] {
  const added = tools.filter((entry) => entry !== null);
  return added.length ? tidy([...items, SEPARATOR, ...added]) : [...items];
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
  shortcuts: Keymap = {},
  trusted = true,
): PaletteCommand[] {
  return commands
    .filter((command) => commandProblem(command) === null)
    .map((command) => ({
      id: toolPaletteId(command),
      title: `${command.source ? "Repository commands" : "Tools"}: ${toolLabel(command, trusted, shortcuts)}`,
      synonyms: ["user command", "external tool", command.program],
      unavailable: blocked ?? (context === null ? "Open a repository first" : (toolBlocked(command, context) ?? undefined)),
      shortcut: shortcuts[toolPaletteId(command)] ? prettyKeys(shortcuts[toolPaletteId(command)]!) : undefined,
      run: () => run(command),
    }));
}

function toolLabel(command: UserCommand, trusted: boolean, keys: Keymap): string {
  if (!trusted) return `${command.name} (review trust…)`;
  if (command.source && command.shortcut && !keys[toolPaletteId(command)]) return `${command.name} (shortcut unavailable)`;
  return command.name;
}

export function toolRequest(command: UserCommand, context: ToolContext): ToolRequest {
  return {
    name: command.name,
    program: command.program,
    args: command.args,
    cwd: command.cwd,
    detached: command.detached,
    confirm: command.confirm,
    source: command.source ?? null,
    output: command.output,
    context,
  };
}

function selectedToolFiles(context: ToolContext): readonly string[] {
  return context.files?.length ? context.files : context.file ? [context.file] : [];
}

function matchSegment(pattern: string, text: string): boolean {
  let at = 0;
  let from = 0;
  let star = -1;
  let resume = 0;
  while (at < text.length) {
    if (pattern[from] === "*") {
      star = from++;
      resume = at;
    } else if (pattern[from] === "?" || pattern[from] === text[at]) {
      at++;
      from++;
    } else if (star >= 0) {
      from = star + 1;
      at = ++resume;
    } else {
      return false;
    }
  }
  while (pattern[from] === "*") from++;
  return from === pattern.length;
}

function matchPath(pattern: string[], path: string): boolean {
  if (pattern.length === 1) return matchSegment(pattern[0]!, fileName(path));
  const segments = path.split("/");
  let matched = new Uint8Array(segments.length + 1);
  matched[0] = 1;
  for (const part of pattern) {
    const next = new Uint8Array(matched.length);
    if (part === "**") next[0] = matched[0]!;
    for (let at = 1; at < next.length; at++) {
      next[at] = part === "**" ? matched[at]! | next[at - 1]! : Number(matched[at - 1] && matchSegment(part, segments[at - 1]!));
    }
    matched = next;
  }
  return matched[segments.length] === 1;
}

export function commandMatchesFiles(command: UserCommand, paths: readonly string[], os: Os = OS): boolean {
  const fold = (path: string) => os === "linux" ? path : path.toLowerCase();
  const patterns = fold(command.patterns).split(";").map((part) => part.trim()).filter(Boolean).map((part) => part.split("/"));
  return patterns.length === 0 || (paths.length > 0 && paths.every((path) => patterns.some((pattern) => matchPath(pattern, fold(path)))));
}

export function mergeToolCommands(existing: readonly UserCommand[], incoming: readonly UserCommand[]): UserCommand[] {
  const merged = [...existing];
  const names = new Set(existing.map((command) => command.name.toLowerCase()));
  for (const command of incoming.slice(0, MAX_USER_COMMANDS - existing.length)) {
    let name = command.name;
    for (let suffix = 2; names.has(name.toLowerCase()); suffix++) name = `${command.name} (${suffix})`;
    names.add(name.toLowerCase());
    const { id } = newUserCommand(merged);
    merged.push({ ...command, name, id });
  }
  return merged;
}

export function toolMenuId(command: UserCommand): string {
  return command.source ? `repo-tool:${command.source.index}` : `${TOOL_MENU_PREFIX}${command.id}`;
}

export function toolPaletteId(command: UserCommand): string {
  return command.source ? `palette-repository-tool:${command.source.index}` : `${TOOL_PALETTE_PREFIX}${command.id}`;
}

export function toolShortcutProblem(command: UserCommand, commands: readonly UserCommand[], builtins: Keymap, onMac = ON_MAC, labels: Record<string, string> = {}): string | null {
  const keys = command.shortcut;
  if (!keys) return null;
  const refused = shortcutRefusal(keys, onMac);
  if (refused) return refused;
  if (!claimable(keys)) return "Cogit cannot take this key; press another.";
  const chord = canonicalShortcut(keys, onMac);
  if (chord === "f6" || chord === "shift+f6") return `${prettyKeys(keys, onMac)} is kept for panel navigation`;
  if ((!onMac && chord.split("+").includes("cmd")) || (onMac && chord.split("+").includes("ctrl") && !/^ctrl\+(shift\+)?(tab|pageup|pagedown)$/.test(chord))) {
    return "Cogit cannot take this key on this platform; press another.";
  }
  const clashes = Object.entries(builtins).filter(([, bound]) => bound && canonicalShortcut(bound, onMac) === chord).map(([id]) => labels[id] ?? id);
  const others = commands.filter((other) => other.id !== command.id && other.shortcut && canonicalShortcut(other.shortcut, onMac) === chord);
  clashes.push(...others.map((other) => other.name));
  return clashes.length ? `Also bound to ${clashes.join(", ")}` : null;
}

export function toolShortcuts(commands: readonly UserCommand[], builtins: Keymap): Keymap {
  return Object.fromEntries(commands.filter((command) => command.shortcut && !toolShortcutProblem(command, commands, builtins)).map((command) => [toolPaletteId(command), command.shortcut]));
}

export function repositoryToolShortcuts(shared: readonly UserCommand[], personal: readonly UserCommand[], builtins: Keymap): Keymap {
  const reserved = { ...builtins, ...Object.fromEntries(personal.map((command) => [toolPaletteId(command), command.shortcut])) };
  return toolShortcuts(shared, reserved);
}

export function commitToolOutput(draft: string, stdout: string, replace: boolean, approved: boolean): string {
  const message = stdout.replaceAll("\r\n", "\n").replace(/\n+$/, "");
  if (!message || (replace && draft !== "" && !approved)) return draft;
  return replace || draft === "" ? message : `${draft}${draft.endsWith("\n") ? "" : "\n"}${message}`;
}
