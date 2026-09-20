import { defaultVisible, type RefNode } from "$lib/ref-nodes";
import { listRemotes, remoteUrl, type RepoId } from "$lib/ipc";

/** Per repository, so ticking `master` in one does not change what another shows. */
const STORAGE_KEY = "cogit.visible-refs.v1";

function stored(): Record<string, string[]> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? {} : (JSON.parse(raw) as Record<string, string[]>);
  } catch {
    return {};
  }
}

class RefsStore {
  visible = $state.raw<ReadonlySet<string>>(new Set());
  collapsed = $state.raw<ReadonlySet<string>>(new Set());
  filter = $state("");
  urls = $state.raw<Record<string, string>>({});

  #root: string | null = null;

  /** Until the tree has been built once there is nothing to seed a default from. */
  adopt(root: string, nodes: readonly RefNode[]): void {
    if (this.#root === root) return;
    this.#root = root;
    const remembered = stored()[root];
    const known = new Set(nodes.map((node) => node.id));
    const kept = (remembered ?? []).filter((id) => known.has(id));
    this.visible = kept.length > 0 ? new Set(kept) : defaultVisible(nodes);
  }

  set(next: Set<string>): void {
    this.visible = next;
    if (this.#root === null) return;
    try {
      const all = stored();
      all[this.#root] = [...next];
      localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
    } catch {
      // A blocked store costs the ticks on restart, nothing more.
    }
  }

  collapse(id: string): void {
    const next = new Set(this.collapsed);
    if (!next.delete(id)) next.add(id);
    this.collapsed = next;
  }

  async loadUrls(repo: RepoId): Promise<void> {
    try {
      const names = await listRemotes(repo);
      const pairs = await Promise.all(
        names.map(async (name) => [name, (await remoteUrl(repo, name)) ?? ""] as const),
      );
      this.urls = Object.fromEntries(pairs.filter(([, url]) => url !== ""));
    } catch {
      this.urls = {};
    }
  }

  clear(): void {
    this.#root = null;
    this.visible = new Set();
    this.collapsed = new Set();
    this.filter = "";
    this.urls = {};
  }
}

export const refs = new RefsStore();
