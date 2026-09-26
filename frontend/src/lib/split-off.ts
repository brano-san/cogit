function plural(count: number): string {
  return count === 1 ? "1 file" : `${count} files`;
}

/** Why the split cannot run, or `null` when it can. The backend checks the same rules. */
export function splitProblem(
  changed: readonly string[],
  chosen: readonly string[],
  message: string,
): string | null {
  if (chosen.length === 0) return "Choose the files to split off.";
  if (chosen.length >= changed.length) {
    return "Leave at least one file in the original commit.";
  }
  if (message.trim() === "") return "The new commit needs a message.";
  return null;
}

export function splitSummary(
  changed: readonly string[],
  chosen: readonly string[],
  splitFirst: boolean,
): string {
  const rest = plural(changed.length - chosen.length);
  const taken = plural(chosen.length);
  return splitFirst
    ? `${taken} before, ${rest} in the original commit`
    : `${rest} in the original commit, ${taken} after`;
}

/** Files ticked or a message typed: work closing the dialog loses. */
export function splitStarted(chosen: readonly string[], message: string): boolean {
  return chosen.length > 0 || message.trim() !== "";
}

export interface SplitRequest {
  oid: string;
  changed: readonly string[];
  published: boolean;
}

/** What Split Off acts on, taken when it opens: whatever is selected in the graph later is
    not the commit the user chose to split. */
export async function splitRequest(
  oid: string,
  read: { files: (oid: string) => Promise<readonly string[]>; published: (oid: string) => Promise<boolean> },
): Promise<SplitRequest> {
  const changed = await read.files(oid);
  return { oid, changed, published: await read.published(oid) };
}
