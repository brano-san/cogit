import { defaultVisible, type RefNode } from "$lib/ref-nodes";
import { listRemotes, remoteUrl, type RepoId } from "$lib/ipc";

/** Per repository, so ticking `master` in one does not change what another shows. */
const STORAGE_KEY = "cogit.visible-refs.v2";

interface Saved {
  visible: string[];
  /** Which headings the user folded away; T5.1 asks for this to survive a restart. */
  collapsed: string[];
}

function stored(): Record<string, Saved> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? {} : (JSON.parse(raw) as Record<string, Saved>);
  } catch {
    return {};
  }
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry) => typeof entry === "string") : [];
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
    const kept = strings(remembered?.visible).filter((id) => known.has(id));
    this.visible = kept.length > 0 ? new Set(kept) : defaultVisible(nodes);
    this.collapsed = new Set(strings(remembered?.collapsed));
  }

  set(next: Set<string>): void {
    this.visible = next;
    this.persist();
  }

  collapse(id: string): void {
    const next = new Set(this.collapsed);
    if (!next.delete(id)) next.add(id);
    this.collapsed = next;
    this.persist();
  }

  private persist(): void {
    if (this.#root === null) return;
    try {
      const all = stored();
      all[this.#root] = { visible: [...this.visible], collapsed: [...this.collapsed] };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
    } catch {
      // A blocked store costs the ticks and the folds on restart, nothing more.
    }
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
