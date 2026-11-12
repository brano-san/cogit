import type { CommitDetails, CommitRow, Signature } from "$lib/ipc";
import { dateTooltip, shortOid } from "./format";
import type { RefNode } from "./ref-nodes";

function when(signature: Signature): string {
  return dateTooltip(signature.timestamp, signature.tzOffsetMinutes);
}

function who(signature: Signature): string {
  return `${signature.name} <${signature.email}>`;
}

function trackingState(ahead: number, behind: number): string {
  if (ahead === 0 && behind === 0) return "identical";
  return [ahead > 0 ? `${ahead} ahead` : null, behind > 0 ? `${behind} behind` : null].filter(Boolean).join(", ");
}

function commitLines(details: CommitDetails): string[] {
  return [
    `Date: ${when(details.author)}`,
    `Author: ${who(details.author)}`,
    `Commit Message: ${details.summary}`,
    `Commit Date: ${when(details.committer)}`,
    `Committer: ${who(details.committer)}`,
    `ID: ${shortOid(details.oid)}`,
  ];
}

export function refTip(node: RefNode, details: CommitDetails | null): string | null {
  const head: string[] = [];
  switch (node.kind) {
    case "local": {
      const branch = node.branch;
      if (!branch) return null;
      head.push(`Local Branch: ${branch.name}`);
      if (branch.upstream) {
        head.push(`Tracked Branch: ${branch.upstream}`, `Tracking State: ${node.detail?.endsWith(": gone") ? "gone" : trackingState(branch.ahead, branch.behind)}`);
      }
      break;
    }
    case "remote":
      if (!node.branch) return null;
      head.push(`Remote Branch: ${node.branch.name}`);
      break;
    case "tag":
      if (!node.tag) return null;
      head.push(`Tag: ${node.tag.name}`);
      break;
    case "stash":
      head.push(`Stash: ${node.rev ?? node.label}`, `Comment: ${node.label}`);
      if (details) head.push(`Commit Message: ${details.summary}`, `ID: ${shortOid(details.oid)}`);
      return head.join("\n");
    case "other":
      return details ? commitLines(details).join("\n") : (node.rev ?? node.label);
    default:
      return null;
  }
  return [...head, ...(details ? commitLines(details) : [])].join("\n");
}

export function rowTip(commit: Omit<CommitRow, "parents">): string {
  const author = { name: commit.authorName, email: commit.authorEmail, timestamp: commit.timestamp, tzOffsetMinutes: commit.tzOffsetMinutes };
  return [`Date: ${when(author)}`, `Author: ${who(author)}`, `Commit Message: ${commit.summary}`, `ID: ${shortOid(commit.oid)}`].join("\n");
}
