function commits(count: number): string {
  return count === 1 ? "1 commit" : `${count} commits`;
}

export function trackTooltip(ahead: number, behind: number): string {
  const parts: string[] = [];
  if (ahead > 0) parts.push(`Ahead: ${commits(ahead)} not on the upstream yet. Push to publish them.`);
  if (behind > 0) parts.push(`Behind: ${commits(behind)} on the upstream not here yet. Pull to get them.`);
  if (ahead > 0 && behind > 0) parts.push("Diverged: pull (merge or rebase) before pushing.");
  return parts.join("\n");
}

export const MISSING_REPOSITORY =
  "Missing: this folder is no longer on disk. Close it here, or open it again from where it moved.";

export const DIRTY_REPOSITORY = "Uncommitted changes in the working tree";
