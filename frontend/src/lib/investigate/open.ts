import { listRepositories, type DiffSpec, type RepoId } from "$lib/ipc";
import { openInvestigateWindow } from "$lib/ipc/investigate";
import { splitSelection } from "$lib/selection";
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

/** A diff selection's line: the first added one, or else a deleted one in the version before. */
export function investigateTarget(
  spec: DiffSpec,
  selected: ReadonlySet<string>,
): { rev: string | null; line: number | null } {
  const { deletes, inserts } = splitSelection(selected);
  const after =
    spec.kind === "commitVsParent" ? spec.oid : spec.kind === "commitVsCommit" ? spec.b : null;
  if (inserts.length > 0) return { rev: after, line: Math.min(...inserts) };
  if (deletes.length > 0) {
    const before =
      spec.kind === "commitVsParent"
        ? `${spec.oid}^`
        : spec.kind === "commitVsCommit"
          ? spec.a
          : "HEAD";
    return { rev: before, line: Math.min(...deletes) };
  }
  return { rev: after, line: null };
}
