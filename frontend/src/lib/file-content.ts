import type { DiffSpec, FileContent, FileEntry, FileStatus } from "$lib/ipc";

/** Show anyway reads up to the cap the backend keeps for any side (20 MiB). */
export const SHOW_ANYWAY_LIMIT = 20 * 1024 * 1024;

/** States with nothing to compare: the file itself is what the Diff panel shows. */
const WHOLE: ReadonlySet<FileStatus> = new Set(["unchanged", "ignored", "assumeUnchanged", "skipped"]);

export function showsWholeFile(status: FileStatus | null): boolean {
  return status !== null && WHOLE.has(status);
}

/** The status of the row the diff was opened from, in the list that spec reads. */
export function statusFor(
  spec: DiffSpec | null,
  path: string | null,
  lists: { unstaged: readonly FileEntry[]; staged: readonly FileEntry[]; commit: readonly FileEntry[] },
): FileStatus | null {
  if (spec === null || path === null) return null;
  const list =
    spec.kind === "workTreeVsIndex"
      ? lists.unstaged
      : spec.kind === "indexVsHead"
        ? lists.staged
        : spec.kind === "commitVsParent"
          ? lists.commit
          : null;
  return list?.find((file) => file.path === path)?.status ?? null;
}

const bytes = (size: number) => `${size.toLocaleString("en-US")} bytes`;

/** What stands in for a file that is not shown as text. */
export function contentNotice(content: Exclude<FileContent, { kind: "text" }>): string {
  switch (content.kind) {
    case "binary":
      return `Binary file, ${bytes(content.size)}`;
    case "tooLarge":
      return `The file is ${bytes(content.size)}, over the limit of ${bytes(content.limit)} for showing it.`;
    case "missing":
      return "The file is not there.";
  }
}
