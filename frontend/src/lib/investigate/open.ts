import { listRepositories, type DiffSpec, type RepoId } from "$lib/ipc";
import { openInvestigateWindow } from "$lib/ipc/investigate";
import { investigateTitle, investigateUrl } from "./params";

/** The one entry into Investigate; `rev: null` is the working tree, `line` is 1-based. */
export async function openInvestigate(
  repo: RepoId,
  path: string,
  rev: string | null,
  line: number | null = null,
): Promise<void> {
  const repoName = await nameOf(repo);
  const url = investigateUrl({ repo, repoName, start: { path, rev, line } });
  await openInvestigateWindow(url, investigateTitle(path, repoName));
}

async function nameOf(repo: RepoId): Promise<string> {
  try {
    return (await listRepositories()).find((entry) => entry.repo === repo)?.name ?? "";
  } catch {
    return "";
  }
}

/** The version a diff's Investigate opens on: the commit a commit's diff shows, else the
    working tree (`null`). */
export function investigateRev(spec: DiffSpec): string | null {
  if (spec.kind === "commitVsParent") return spec.oid;
  return spec.kind === "commitVsCommit" ? spec.b : null;
}
