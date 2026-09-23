import { fileName, type Location } from "./params";

/** Back and Forward over the places visited, as a browser keeps them. */
export interface History {
  entries: readonly Location[];
  at: number;
}

export const HISTORY_LIMIT = 100;

export function startHistory(location: Location): History {
  return { entries: [location], at: 0 };
}

export function current(history: History): Location {
  return history.entries[history.at] ?? history.entries[0]!;
}

/** A new step drops whatever Forward held. The same file and version is not a new
    step, only a different line of it: that replaces the current entry instead. */
export function pushLocation(history: History, location: Location): History {
  const here = current(history);
  if (here.path === location.path && here.rev === location.rev) {
    return replaceCurrent(history, location);
  }
  const kept = history.entries.slice(0, history.at + 1);
  kept.push(location);
  const entries = kept.slice(Math.max(0, kept.length - HISTORY_LIMIT));
  return { entries, at: entries.length - 1 };
}

export function replaceCurrent(history: History, location: Location): History {
  const entries = [...history.entries];
  entries[history.at] = location;
  return { entries, at: history.at };
}

export function canGoBack(history: History): boolean {
  return history.at > 0;
}

export function canGoForward(history: History): boolean {
  return history.at < history.entries.length - 1;
}

export function goTo(history: History, index: number): History {
  const at = Math.min(Math.max(index, 0), history.entries.length - 1);
  return { entries: history.entries, at };
}

export function goBack(history: History): History {
  return goTo(history, history.at - 1);
}

export function goForward(history: History): History {
  return goTo(history, history.at + 1);
}

/** The Back dropdown: the places before this one, most recent first. */
export function backList(history: History): { index: number; location: Location }[] {
  return history.entries
    .slice(0, history.at)
    .map((location, index) => ({ index, location }))
    .reverse();
}

export function locationLabel(location: Location): string {
  const version = location.rev ? location.rev.slice(0, 7) : "Working Tree";
  const line = location.line ? ` · line ${location.line}` : "";
  return `${fileName(location.path)} @ ${version}${line}`;
}
