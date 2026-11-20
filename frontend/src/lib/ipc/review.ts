import { Channel } from "@tauri-apps/api/core";
import { commands, type DiffSpec, type FileEntry, type RepoId, type ReviewCommit, type ReviewPath } from "./bindings";
import { unwrap } from "./index";

export type { BranchReview, LineCounts, ReviewCommit, ReviewPath } from "./bindings";

export async function reviewBase(repo: RepoId, branch: string) {
  return unwrap(await commands.reviewBase(repo, branch));
}

export async function branchReview(repo: RepoId, base: string, branch: string) {
  const commits: ReviewCommit[] = [];
  const summary = await stream(
    async (channel: Channel<ReviewCommit[]>) => unwrap(await commands.branchReview(repo, base, branch, channel)),
    (review) => review.ahead,
    (chunk) => commits.push(...chunk),
  );
  return { ...summary, commits };
}

export function reviewFiles(
  repo: RepoId,
  mergeBase: string,
  branch: string,
  worktree: boolean,
  onchunk: (files: FileEntry[]) => void,
): Promise<number> {
  return stream(
    async (channel) => unwrap(await commands.reviewFiles(repo, mergeBase, branch, worktree, channel)),
    (total) => total,
    onchunk,
  );
}

export async function reviewCommitFiles(repo: RepoId, oid: string): Promise<FileEntry[]> {
  const files: FileEntry[] = [];
  await stream(
    async (channel: Channel<FileEntry[]>) => unwrap(await commands.reviewCommitFiles(repo, oid, channel)),
    (total) => total,
    (chunk) => files.push(...chunk),
  );
  return files;
}

function stream<T, Result>(
  run: (channel: Channel<T[]>) => Promise<Result>,
  count: (result: Result) => number,
  onchunk: (chunk: T[]) => void,
): Promise<Result> {
  return new Promise((resolve, reject) => {
    let received = 0;
    let result: Result | undefined;
    const settle = () => {
      if (result !== undefined && received >= count(result)) resolve(result);
    };
    const channel = new Channel<T[]>();
    channel.onmessage = (chunk) => {
      received += chunk.length;
      onchunk(chunk);
      settle();
    };
    run(channel)
      .then((value) => {
        result = value;
        settle();
      })
      .catch(reject);
  });
}

export async function reviewLineCounts(repo: RepoId, spec: DiffSpec, files: ReviewPath[]) {
  return unwrap(await commands.reviewLineCounts(repo, spec, files));
}

export async function openReviewWindow(url: string, title: string) {
  return unwrap(await commands.openReviewWindow(url, title));
}
