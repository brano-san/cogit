import { Channel } from "@tauri-apps/api/core";
import { commands, type DiffSpec, type FileEntry, type RepoId, type ReviewPath } from "./bindings";
import { unwrap } from "./index";

export type { BranchReview, LineCounts, ReviewCommit, ReviewPath } from "./bindings";

export async function reviewBase(repo: RepoId, branch: string) {
  return unwrap(await commands.reviewBase(repo, branch));
}

export async function branchReview(repo: RepoId, base: string, branch: string) {
  return unwrap(await commands.branchReview(repo, base, branch));
}

export function reviewFiles(
  repo: RepoId,
  mergeBase: string,
  branch: string,
  worktree: boolean,
  onchunk: (files: FileEntry[]) => void,
): Promise<number> {
  return new Promise((resolve, reject) => {
    let received = 0;
    let total: number | null = null;
    const settle = () => {
      if (total !== null && received >= total) resolve(total);
    };
    const channel = new Channel<FileEntry[]>();
    channel.onmessage = (chunk) => {
      received += chunk.length;
      onchunk(chunk);
      settle();
    };
    commands
      .reviewFiles(repo, mergeBase, branch, worktree, channel)
      .then((result) => {
        total = unwrap(result);
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
