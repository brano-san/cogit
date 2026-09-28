import { commands } from "./bindings";
import type { CommitRow, RepoId } from "./bindings";
import { collect } from "./investigate";

/** Every lost commit, newest first, handed over chunk by chunk as they arrive. */
export function lostCommits(repo: RepoId, onChunk: (rows: CommitRow[]) => void): Promise<void> {
  let received = 0;
  return collect<void, CommitRow[]>(
    (channel) => commands.lostCommits(repo, channel),
    (chunk) => {
      received += chunk.length;
      onChunk(chunk);
    },
    () => received,
    () => undefined,
  );
}
