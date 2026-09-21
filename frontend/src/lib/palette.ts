import type { PanelId, PerspectiveId } from "./perspectives";

export interface PaletteCommand {
  id: string;
  title: string;
  shortcut?: string;
  synonyms?: string[];
  /** Why the command cannot run now. Shown, not hidden: hiding it teaches nothing. */
  unavailable?: string;
  run: () => void;
}

const WORD_BOUNDARY_BONUS = 8;
const CONSECUTIVE_BONUS = 4;
const BASE = 1;

/** Subsequence match; a higher score means a better one, `0` means no match. */
export function fuzzyScore(title: string, query: string): number {
  const haystack = title.toLowerCase();
  const needle = query.trim().toLowerCase();
  if (needle === "") return BASE;

  let score = 0;
  let at = 0;
  let previous = -2;

  for (const letter of needle) {
    const found = haystack.indexOf(letter, at);
    if (found === -1) return 0;
    score += BASE;
    if (found === 0 || haystack[found - 1] === " ") score += WORD_BOUNDARY_BONUS;
    if (found === previous + 1) score += CONSECUTIVE_BONUS;
    previous = found;
    at = found + 1;
  }
  return score;
}

export function rankCommands(
  commands: readonly PaletteCommand[],
  query: string,
  recent: readonly string[],
): PaletteCommand[] {
  const scored = commands
    .map((command) => {
      const best = Math.max(
        fuzzyScore(command.title, query),
        ...(command.synonyms ?? []).map((synonym) => fuzzyScore(synonym, query)),
      );
      return { command, score: best };
    })
    .filter((entry) => entry.score > 0);

  scored.sort((a, b) => {
    const blocked = Number(Boolean(a.command.unavailable)) - Number(Boolean(b.command.unavailable));
    if (blocked !== 0) return blocked;
    if (query.trim() === "") {
      const recency = recent.indexOf(a.command.id) - recent.indexOf(b.command.id);
      const seen = (id: string) => (recent.includes(id) ? 0 : 1);
      const bySeen = seen(a.command.id) - seen(b.command.id);
      if (bySeen !== 0) return bySeen;
      if (recency !== 0) return recency;
    }
    return b.score - a.score;
  });

  return scored.map((entry) => entry.command);
}

/** Sorted so the native menu only updates when the set really changed. */
export function disabledIds(commands: readonly PaletteCommand[]): string[] {
  return commands
    .filter((command) => command.unavailable !== undefined)
    .map((command) => command.id)
    .sort();
}

export interface ToggleState {
  panels: readonly PanelId[];
  output: boolean;
  maximized: boolean;
  overlap: boolean;
  avatars: boolean;
  perspective: PerspectiveId;
}

/** Sorted, like `disabledIds`: the native menu only needs writing when the set changed. */
export function checkedIds(state: ToggleState): string[] {
  const ids = state.panels.map((panel) => `panel-${panel}`);
  if (state.output) ids.push("output");
  if (state.maximized) ids.push("maximize-panel");
  if (state.overlap) ids.push("overlap");
  if (state.avatars) ids.push("avatars");
  ids.push(`perspective-${state.perspective}`);
  return ids.sort();
}
