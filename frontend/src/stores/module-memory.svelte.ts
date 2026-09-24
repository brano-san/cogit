import {
  forgetRoot,
  readModuleMemory,
  withNode,
  withTop,
  type ModuleMemory,
} from "$lib/module-memory";

const STORAGE_KEY = "cogit.submodule-trees.v1";

function stored(): unknown {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

/** One record of the open submodule trees, read by the tree of the repository on screen
    and by the light trees of every other one, so the two can never disagree (R-352). */
class ModuleMemoryStore {
  memory = $state.raw<ModuleMemory>(readModuleMemory(stored()));

  isOpen(root: string): boolean {
    return this.memory.open.includes(root);
  }

  expanded(root: string): ReadonlySet<string> {
    return new Set(this.memory.nodes[root] ?? []);
  }

  setOpen(root: string, open: boolean): void {
    this.#write(withTop(this.memory, root, open));
  }

  setNode(root: string, key: string, open: boolean): void {
    this.#write(withNode(this.memory, root, key, open));
  }

  forget(root: string): void {
    this.#write(forgetRoot(this.memory, root));
  }

  #write(next: ModuleMemory): void {
    if (next === this.memory) return;
    this.memory = next;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      // A blocked store costs the open trees on restart, nothing more.
    }
  }
}

export const moduleMemory = new ModuleMemoryStore();
