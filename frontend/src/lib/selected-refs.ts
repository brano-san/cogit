import type { RefLabel } from "$lib/format";
import type { Branch } from "$lib/ipc";

/** The Branches tree id of the ref a label draws: the ids `refs.visible` ticks. */
function idsOf(label: RefLabel): string[] {
  switch (label.kind) {
    case "head":
      return ["HEAD", `local:${label.name ?? label.text}`];
    case "local":
      return [`local:${label.name ?? label.text}`];
    case "remote":
      return [`remote:${label.text}`];
    case "tag":
      return [`tag:${label.text}`];
    case "stash":
      return [`stash:${label.text.slice("stash@{".length, -1)}`];
  }
}

/** A branch and the remote copies of it on the same commit share a label (`origin=x`):
    ticking any of them shows it. */
function twinIds(label: RefLabel): string[] {
  const name = label.name ?? label.text;
  return (label.remotes ?? []).map((remote) => `remote:${remote}/${name}`);
}

/** Show Only Selected Branches and Tags (F-561): only the labels of refs ticked in
    Branches, as SmartGit hides the rest. */
export function selectedLabels(labels: readonly RefLabel[], ticked: ReadonlySet<string>): RefLabel[] {
  return labels.filter((label) => [...idsOf(label), ...twinIds(label)].some((id) => ticked.has(id)));
}

/** Include Tracked Remote Branches (F-561): a ticked branch brings the remote branch it
    tracks into the graph, HEAD's branch too while HEAD is ticked, as SmartGit does. */
export function withTracked(
  ticked: ReadonlySet<string>,
  branches: readonly Pick<Branch, "name" | "kind" | "upstream" | "isHead">[],
): ReadonlySet<string> {
  const remotes = new Set(branches.filter((branch) => branch.kind === "remote").map((branch) => branch.name));
  const added = branches
    .filter((branch) => branch.kind === "local" && branch.upstream !== null && remotes.has(branch.upstream))
    .filter((branch) => ticked.has(`local:${branch.name}`) || (branch.isHead && ticked.has("HEAD")))
    .map((branch) => `remote:${branch.upstream}`)
    .filter((id) => !ticked.has(id));
  return added.length === 0 ? ticked : new Set([...ticked, ...added]);
}
