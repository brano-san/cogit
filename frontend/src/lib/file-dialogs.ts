export interface RemoveRow {
  path: string;
  name: string;
  directory: string;
}

export function removeRows(paths: readonly string[]): RemoveRow[] {
  return [...new Set(paths)]
    .sort((a, b) => a.localeCompare(b))
    .map((path) => {
      const bare = path.replace(/\/+$/, "");
      const cut = bare.lastIndexOf("/");
      return { path, name: bare.slice(cut + 1), directory: cut < 0 ? "" : bare.slice(0, cut) };
    });
}

const LISTED = 12;

/** A question about several files names them under it, as far as a dialog can hold. */
export function listedMessage(question: string, paths: readonly string[]): string {
  if (paths.length < 2) return question;
  const shown = paths.slice(0, LISTED);
  const rest = paths.length - shown.length;
  return [question, "", ...shown, ...(rest > 0 ? [`and ${rest} more`] : [])].join("\n");
}

/** One pane of the Index Editor. A textarea only ever holds `\n`, so the side remembers
    the ending of each of its lines and gives it back on save. */
export interface EditorSide {
  text: string;
  /** `endings[i]` ends line `i`: `\r\n` or `\n`. */
  endings: readonly string[];
  present: boolean;
}

export function editorSide(raw: string | null): EditorSide {
  if (raw === null) return { text: "", endings: [], present: false };
  const endings = [...raw.matchAll(/\r?\n/g)].map((match) => match[0]);
  return { text: raw.replaceAll("\r\n", "\n"), endings, present: true };
}

/** The lines kept at the start and at the end keep their own endings; an edited line takes
    the ending of the line it replaced, a new one that of the line above it. */
export function forDisk(side: EditorSide, text: string): string {
  const lines = text.replaceAll("\r\n", "\n").split("\n");
  const before = side.text.split("\n");
  const ends = side.endings;
  let head = 0;
  while (head < lines.length && head < before.length && lines[head] === before[head]) head += 1;
  const room = Math.min(lines.length, before.length) - head;
  let tail = 0;
  while (tail < room && lines[lines.length - 1 - tail] === before[before.length - 1 - tail]) tail += 1;

  const crlf = ends.filter((ending) => ending === "\r\n").length;
  let previous = crlf > ends.length - crlf ? "\r\n" : "\n";
  let out = lines[0] ?? "";
  for (let at = 0; at < lines.length - 1; at += 1) {
    const kept = at >= lines.length - tail ? at - lines.length + before.length : at;
    const known = at < head || at >= lines.length - tail || at < before.length - tail;
    const ending = (known ? ends[kept] : undefined) ?? previous;
    out += ending + (lines[at + 1] ?? "");
    previous = ending;
  }
  return out;
}

/** What Save writes: `null` for a side nobody touched. */
export function editedSides(
  sides: { index: EditorSide; worktree: EditorSide },
  texts: { index: string; worktree: string },
): { index: string | null; worktree: string | null } {
  const pick = (side: EditorSide, text: string) =>
    text.replaceAll("\r\n", "\n") === side.text ? null : forDisk(side, text);
  return { index: pick(sides.index, texts.index), worktree: pick(sides.worktree, texts.worktree) };
}
