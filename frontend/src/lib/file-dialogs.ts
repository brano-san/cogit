
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

/** One pane of the Index Editor. A textarea only ever holds `\n`, so a file written with
    `\r\n` remembers it here and gets it back on save. */
export interface EditorSide {
  text: string;
  crlf: boolean;
  present: boolean;
}

export function editorSide(raw: string | null): EditorSide {
  if (raw === null) return { text: "", crlf: false, present: false };
  return { text: raw.replaceAll("\r\n", "\n"), crlf: raw.includes("\r\n"), present: true };
}

export function forDisk(side: EditorSide, text: string): string {
  const plain = text.replaceAll("\r\n", "\n");
  return side.crlf ? plain.replaceAll("\n", "\r\n") : plain;
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
