import type { DiffSpec, FileDiff } from "$lib/ipc";

/** Whether the Diff panel offers Edit, and why it is off: `null` hides the button. Only the
    working tree side is a file one can write; a staged or committed side never is. */
export function editOffer(spec: DiffSpec | null, diff: FileDiff): { blocked: string | null } | null {
  if (spec?.kind !== "workTreeVsIndex" && spec?.kind !== "commitVsWorkTree") return null;
  if (diff.kind === "binary") return { blocked: "A binary file cannot be edited here." };
  if (diff.kind === "tooLarge") return { blocked: "The file is too large to edit here; use your editor." };
  if (diff.kind !== "text" && diff.kind !== "whitespaceOnly" && diff.kind !== "eolOnly") return null;
  if (diff.kind === "text" && diff.converted)
    return { blocked: `Shown converted (${diff.converted}): what is shown is not the bytes on disk.` };
  if (diff.kind === "text" && diff.lossyEncoding)
    return { blocked: "The file's encoding did not decode cleanly; edit it in your editor." };
  return { blocked: null };
}
