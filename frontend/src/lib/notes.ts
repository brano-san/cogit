import type { CommitNote } from "$lib/ipc/bindings";

/** A note's tooltip text: the default namespace bare, any other under its name. Plain text
    only; the tooltip layer never renders it as HTML. */
export function noteTip(notes: readonly CommitNote[]): string {
  const shown = notes.filter((note) => note.text.trim() !== "");
  const [only] = shown;
  if (shown.length === 1 && only?.namespace === "commits") return only.text;
  return shown.map((note) => `${note.namespace}:\n${note.text}`).join("\n\n");
}
