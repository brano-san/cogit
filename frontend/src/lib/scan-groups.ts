import type { ScanHit } from "$lib/ipc";

export type ScanLayout = "folders" | "flat";

export interface ScanSection {
  /** `null`: the flat list, drawn without a heading. */
  title: string | null;
  hits: ScanHit[];
}

const slashed = (path: string) => path.replace(/\\/g, "/").replace(/\/+$/, "");

/** The folder a hit sits in, relative to the scanned one; the scanned folder's own name for
    a repository directly inside it. */
function parentOf(root: string, base: string): string {
  const path = slashed(root);
  const top = slashed(base);
  const inside = path.toLowerCase().startsWith(`${top.toLowerCase()}/`) ? path.slice(top.length + 1) : path;
  const cut = inside.lastIndexOf("/");
  return cut > 0 ? inside.slice(0, cut) : (top.split("/").pop() ?? top);
}

/** Search for repositories, as drawn: what can be opened first — one list, or a group per
    folder in name order — then bare repositories and those already open, each in a group of
    its own below, so they do not crowd the rest. */
export function scanSections(hits: readonly ScanHit[], layout: ScanLayout, base: string): ScanSection[] {
  const main = hits.filter((hit) => !hit.bare && !hit.alreadyOpen);
  const sections: ScanSection[] = [];
  if (layout === "flat") {
    if (main.length > 0) sections.push({ title: null, hits: main });
  } else {
    const folders = new Map<string, ScanHit[]>();
    for (const hit of main) {
      const folder = parentOf(hit.root, base);
      folders.set(folder, [...(folders.get(folder) ?? []), hit]);
    }
    const names = [...folders.keys()].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
    for (const name of names) sections.push({ title: name, hits: folders.get(name) ?? [] });
  }
  const bare = hits.filter((hit) => hit.bare && !hit.alreadyOpen);
  const open = hits.filter((hit) => hit.alreadyOpen);
  if (bare.length > 0) sections.push({ title: "Bare", hits: bare });
  if (open.length > 0) sections.push({ title: "Already open", hits: open });
  return sections;
}
