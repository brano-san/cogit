import { PANELS, type PanelId } from "./perspectives";

/**
 * Which panel the keyboard is talking to. Exactly one, and only ever a visible one:
 * cycling onto a hidden panel would look like the key did nothing (issue 15).
 */

/** The next visible panel after `from`, wrapping round. `from` itself if it is alone. */
export function step(
  from: PanelId,
  visible: (panel: PanelId) => boolean,
  delta: 1 | -1,
): PanelId {
  const shown = PANELS.filter(visible);
  if (shown.length === 0) return from;

  const at = shown.indexOf(from);
  if (at === -1) return shown[0] as PanelId;

  const next = (at + delta + shown.length) % shown.length;
  return shown[next] as PanelId;
}

/** Where focus lands when the current panel disappears, or nothing has it yet. */
export function settle(current: PanelId, visible: (panel: PanelId) => boolean): PanelId {
  if (visible(current)) return current;
  return PANELS.find(visible) ?? current;
}

/**
 * Select All is per-panel, and the graph is the one place it is refused: a repository
 * with fifty thousand commits would select every one of them, which helps nobody and
 * costs a long freeze. SmartGit refuses it there too.
 */
export function allowsSelectAll(panel: PanelId): boolean {
  return panel !== "graph";
}
