import type { ConfirmRequest } from "$stores/confirm.svelte";
import { branchRevision, type BranchLike } from "./toolbar";

/** Several branches merged into the current one at once (`git merge a b c`, F-952). */

export type MergeSourceKind = "branch" | "remote" | "tag" | "commit";

export interface MergeSource {
  /** What git is given: a branch shadowed by a tag goes by its full name. */
  rev: string;
  /** What the message and the dialog call it. */
  name: string;
  kind: MergeSourceKind;
}

export interface MergeSources {
  sources: MergeSource[];
  /** Selected but not merged, with the reason. */
  skipped: string[];
}

type SourceNode = {
  kind: string;
  current?: boolean;
  label: string;
  oid?: string;
  branch?: Pick<BranchLike, "name" | "fullName" | "kind">;
  tag?: { name: string };
};

/** The rows of Branches a merge takes, in their order: local and remote branches and tags,
    not the current branch and nothing twice. */
export function branchMergeSources(
  nodes: readonly SourceNode[],
  branches: readonly Pick<BranchLike, "name" | "kind">[],
  tags: readonly { name: string }[],
): MergeSources {
  const sources: MergeSource[] = [];
  const skipped: string[] = [];
  const seen = new Set<string>();
  const add = (source: MergeSource) => {
    if (seen.has(source.rev)) return;
    seen.add(source.rev);
    sources.push(source);
  };
  for (const node of nodes) {
    if (node.kind === "local" && node.branch) {
      if (node.current) skipped.push(`${node.branch.name}, the current branch`);
      else add({ rev: branchRevision(node.branch, branches, tags), name: node.branch.name, kind: "branch" });
    } else if (node.kind === "remote" && node.branch && node.label !== "HEAD") {
      add({ rev: branchRevision(node.branch, branches, tags), name: node.branch.name, kind: "remote" });
    } else if (node.kind === "tag" && node.tag) {
      add({ rev: node.tag.name, name: node.tag.name, kind: "tag" });
    }
  }
  return { sources, skipped };
}

/** A selected commit of the graph as a merge source: a branch on it reads better than a
    hash; a local branch wins over a remote one, HEAD's own counts for nothing. */
export function commitMergeSource(
  oid: string,
  branches: readonly BranchLike[],
  tags: readonly { name: string }[],
): MergeSource {
  const at = branches.filter((branch) => branch.oid === oid && !branch.isHead);
  const branch = at.find((entry) => entry.kind === "local") ?? at.find((entry) => entry.kind === "remote");
  if (!branch) return { rev: oid, name: oid, kind: "commit" };
  return {
    rev: branchRevision(branch, branches, tags),
    name: branch.name,
    kind: branch.kind === "local" ? "branch" : "remote",
  };
}

/** The graph's selection: HEAD's own commit is already merged, and two rows of one commit
    are one source. */
export function commitMergeSources(
  oids: readonly string[],
  head: string | null,
  source: (oid: string) => MergeSource,
): MergeSources {
  const skipped: string[] = [];
  const sources: MergeSource[] = [];
  const seen = new Set<string>();
  for (const oid of oids) {
    if (oid === head) {
      skipped.push(`${oid.slice(0, 7)}, the current commit`);
      continue;
    }
    const each = source(oid);
    if (seen.has(each.rev)) continue;
    seen.add(each.rev);
    sources.push(each);
  }
  return { sources, skipped };
}

const GROUPS: readonly [MergeSourceKind, string, string][] = [
  ["branch", "branch", "branches"],
  ["remote", "remote-tracking branch", "remote-tracking branches"],
  ["tag", "tag", "tags"],
  ["commit", "commit", "commits"],
];

function joined(names: readonly string[]): string {
  const quoted = names.map((name) => `'${name}'`);
  if (quoted.length < 2) return quoted.join("");
  return `${quoted.slice(0, -1).join(", ")} and ${quoted.at(-1)}`;
}

/** The message `git merge` writes for these sources (`fmt-merge-msg`): branches, then
    remote-tracking branches, tags and commits; ` into <branch>` unless it is main or master.
    `into` null: HEAD is detached. */
export function octopusMessage(sources: readonly MergeSource[], into: string | null): string {
  let text = "Merge ";
  let separator = "";
  for (const [kind, one, many] of GROUPS) {
    const names = sources.filter((source) => source.kind === kind).map((source) => source.name);
    if (names.length === 0) continue;
    // git sets commits off with a semicolon: `tag 'v1'; commit '…'`.
    text += kind === "commit" && separator !== "" ? "; " : separator;
    text += `${names.length === 1 ? one : many} ${joined(names)}`;
    separator = ", ";
  }
  const target = into ?? "HEAD";
  if (target !== "main" && target !== "master") text += ` into ${target}`;
  return text;
}

/** Why the selection cannot be merged at once, or null. One source is an ordinary merge. */
export function octopusBlocked(picked: MergeSources): string | null {
  if (picked.sources.length > 0) return null;
  return picked.skipped.length > 0 ? "only the current branch" : "no branch or commit selected";
}

const LABEL: Record<MergeSourceKind, string> = {
  branch: "branch",
  remote: "remote-tracking branch",
  tag: "tag",
  commit: "commit",
};

export function octopusQuestion(picked: MergeSources, into: string | null, message: string): ConfirmRequest {
  const count = picked.sources.length;
  const lines = [
    `Merge ${count} branches into ${into ?? "HEAD"} with one merge commit of ${count + 1} parents?`,
    "An octopus merge cannot stop on conflicts: if any branch conflicts, Git changes nothing " +
      "and the branches have to be merged one by one.",
  ];
  if (picked.skipped.length > 0) lines.push(`Not included: ${picked.skipped.join("; ")}.`);
  return {
    title: "Octopus Merge",
    message: lines.join("\n\n"),
    items: picked.sources.map((source) => `${LABEL[source.kind]} ${source.name}`),
    fields: [
      ["Into", into ?? "HEAD"],
      ["Message", message],
    ],
    option: "Create a merge commit even if Git could fast-forward (--no-ff)",
    confirm: "Merge",
  };
}
