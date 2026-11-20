import type { RepoId } from "$lib/ipc";

export interface ReviewRequest {
  repo: RepoId;
  repoName: string;
  branch: string;
  base: string | null;
}

export function reviewUrl(request: ReviewRequest): string {
  const params = new URLSearchParams({
    repo: String(request.repo),
    name: request.repoName,
    branch: request.branch,
  });
  if (request.base) params.set("base", request.base);
  return `review.html?${params.toString()}`;
}

export function parseReview(search: string): ReviewRequest | null {
  const params = new URLSearchParams(search);
  const repo = Number(params.get("repo"));
  const branch = params.get("branch");
  if (!Number.isInteger(repo) || repo <= 0 || !branch) return null;
  return { repo, repoName: params.get("name") ?? "", branch, base: params.get("base") || null };
}

export function reviewTitle(branch: string, repoName: string): string {
  const repo = repoName ? ` [${repoName}]` : "";
  return `${branch}${repo} - Review Branch`;
}
