import {
  atLaunch,
  forgetRoot,
  readModuleMemory,
  withNode,
  withOpenedRepository,
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
  memory = $state.raw<ModuleMemory>(atLaunch(readModuleMemory(stored())));
  /** Submodules opened in the panels this session, by folder: only these are read in the
      background and drawn open; every other one is closed, as at launch (R-791). */
  openModules = $state.raw<ReadonlySet<string>>(new Set());

  moduleOpen(root: string): boolean {
    return this.openModules.has(root);
  }

  openModule(root: string): void {
    if (!this.openModules.has(root)) this.openModules = new Set([...this.openModules, root]);
  }

  closeModule(root: string): void {
    if (!this.openModules.has(root)) return;
    this.openModules = new Set([...this.openModules].filter((each) => each !== root));
  }

  /** A repository closes, and every submodule in it with it. */
  closeModulesUnder(root: string): void {
    const top = root.replace(/[/\\]+$/, "");
    const kept = [...this.openModules].filter((each) => !each.startsWith(`${top}/`));
    if (kept.length !== this.openModules.size) this.openModules = new Set(kept);
  }

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

  /** Opened by the user just now, not switched to or restored (R-546). */
  opened(root: string): void {
    this.#write(withOpenedRepository(this.memory, root));
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
