import type { ConflictStages, EntryKind } from "$lib/ipc";

type Side = "base" | "ours" | "theirs";

/** What a conflict that is taken whole says it is: a link or a submodule is not "binary". */
export function wholeReason(kind: EntryKind, binary: boolean): string {
  if (kind === "submodule") {
    return "This is a submodule: its folder is not merged. Keep the commit one side points to.";
  }
  if (kind === "symlink") {
    return "This is a symbolic link: it is kept whole from one side, never edited as text.";
  }
  return binary
    ? "This file is binary or not valid UTF-8, so it cannot be merged or edited as text."
    : "This file is larger than 4 MiB, so it is not merged line by line.";
}

/** The commit a submodule side points to, short; the index stage is the gitlink itself. */
export function submoduleCommit(stages: ConflictStages | null, side: Side): string | null {
  const oid = stages?.[side] ?? null;
  return oid ? oid.slice(0, 7) : null;
}

/** "Submodule: ours" over "Ours" for a gitlink; the plain name otherwise. */
export function sideTitle(kind: EntryKind, side: Side): string {
  const name = side.charAt(0).toUpperCase() + side.slice(1);
  return kind === "submodule" ? `Submodule: ${side}` : name;
}

export function externalToolRefusal(kind: EntryKind): string | undefined {
  return kind === "submodule"
    ? "A merge tool cannot merge a submodule: keep ours or theirs"
    : undefined;
}
