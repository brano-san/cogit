import { isAncestor, type RepoId } from "$lib/ipc";
import { confirmation, type ConfirmRequest } from "$stores/confirm.svelte";

export type Answer = "confirm" | "alternative" | null;

export function mergeQuestion(source: string, into: string): ConfirmRequest {
  return {
    title: "Merge",
    message:
      `${into} has no commits that ${source} lacks, so the merge can fast-forward.\n\n` +
      `Fast-forward moves ${into} to ${source} and keeps the history a straight line. ` +
      `A merge commit records that ${source} was merged, even though nothing needs combining.`,
    fields: [
      ["Merge", source],
      ["Into", into],
    ],
    confirm: "Fast-Forward",
    alternative: "Create Merge Commit",
  };
}

/** `noFastForward` for the merge, asked only when a fast-forward is possible; `null` when
    the user canceled. */
export async function chooseMerge(
  source: string,
  into: string,
  canFastForward: () => Promise<boolean>,
  ask: (request: ConfirmRequest) => Promise<Answer>,
): Promise<{ noFastForward: boolean } | null> {
  if (!(await canFastForward().catch(() => false))) return { noFastForward: false };
  const answer = await ask(mergeQuestion(source, into));
  return answer === null ? null : { noFastForward: answer === "alternative" };
}

export function askMerge(repo: RepoId, source: string, into: string) {
  return chooseMerge(
    source,
    into,
    () => isAncestor(repo, "HEAD", source),
    (request) => confirmation.askWithAlternative(request),
  );
}
