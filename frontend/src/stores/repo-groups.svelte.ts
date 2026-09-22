import {
  UNGROUPED,
  addGroup,
  assign,
  mergeGroups,
  nest,
  removeGroup,
  renameGroup,
  type RepoGroups,
} from "$lib/repo-groups";

const STORAGE_KEY = "cogit.repo-groups.v1";

function stored(): unknown {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

class RepoGroupsStore {
  groups = $state.raw<RepoGroups>(mergeGroups(stored()));
  /** Opened this run only: every group, the ungrouped one too, starts folded (R-160). */
  #expanded = $state.raw<ReadonlySet<string>>(new Set());

  get collapsed(): ReadonlySet<string> {
    return new Set([...this.groups.order, UNGROUPED].filter((id) => !this.#expanded.has(id)));
  }

  add(name: string): void {
    this.write(addGroup(this.groups, name).groups);
  }

  rename(id: string, name: string): void {
    this.write(renameGroup(this.groups, id, name));
  }

  remove(id: string): void {
    this.write(removeGroup(this.groups, id));
  }

  /** A group dropped on another becomes its child; dropped on nothing, it comes back up. */
  nest(id: string, parent: string | null): void {
    this.write(nest(this.groups, id, parent));
  }

  assign(root: string, group: string): void {
    this.write(assign(this.groups, root, group));
  }

  collapse(id: string): void {
    const next = new Set(this.#expanded);
    if (!next.delete(id)) next.add(id);
    this.#expanded = next;
  }

  private write(next: RepoGroups): void {
    this.groups = next;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      // A blocked store costs the grouping on restart, nothing more.
    }
  }
}

export const repoGroups = new RepoGroupsStore();
