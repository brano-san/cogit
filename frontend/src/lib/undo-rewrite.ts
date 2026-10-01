import type { UndoRewrite } from "$lib/ipc/ref-ops";

const REFUSED = "There is no merge, rebase or reset to undo: this repository has no ORIG_HEAD yet.";

/** The shared confirm dialog's text for Undo Last Merge / Rebase / Reset; `null` when
    ORIG_HEAD is missing or already is HEAD, with the reason to show instead. */
export function undoRewriteQuestion(
  info: UndoRewrite | null,
): { message: string; warning: boolean } | { refused: string } {
  if (info === null) return { refused: REFUSED };
  if (info.orig === info.head) {
    return { refused: "ORIG_HEAD is where HEAD already is: there is nothing to undo." };
  }
  const moved =
    `Move the current branch back from ${info.head.slice(0, 7)} to ${info.orig.slice(0, 7)} ` +
    "(ORIG_HEAD), where it was before the last merge, rebase or reset.";
  const kept = info.dirty
    ? "\n\nYou have uncommitted changes. They stay in your files (staged ones may become " +
      "unstaged); if the move would overwrite one of them, git refuses and nothing changes."
    : "";
  return {
    message: `${moved}${kept}\n\nThe commits it leaves behind stay in Lost Commits, and Undo Last Operation takes this back.`,
    warning: info.dirty,
  };
}
