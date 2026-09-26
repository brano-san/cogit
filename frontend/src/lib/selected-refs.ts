import type { RefLabel } from "$lib/format";

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
