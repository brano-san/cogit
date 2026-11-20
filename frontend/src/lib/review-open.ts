import { listRepositories, type RepoId } from "$lib/ipc";
import { openReviewWindow } from "$lib/ipc/review";
import { reviewTitle, reviewUrl } from "$lib/review-params";

export async function openReview(repo: RepoId, branch: string, base: string | null = null): Promise<void> {
  const repoName = await nameOf(repo);
  await openReviewWindow(reviewUrl({ repo, repoName, branch, base }), reviewTitle(branch, repoName));
}

async function nameOf(repo: RepoId): Promise<string> {
  try {
    return (await listRepositories()).find((entry) => entry.repo === repo)?.name ?? "";
  } catch {
    return "";
  }
}
