import {
  addGroup,
  assign,
  mergeGroups,
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
  collapsed = $state.raw<ReadonlySet<string>>(new Set());

  add(name: string): void {
    this.write(addGroup(this.groups, name).groups);
  }

  rename(id: string, name: string): void {
    this.write(renameGroup(this.groups, id, name));
  }

  remove(id: string): void {
    this.write(removeGroup(this.groups, id));
  }

  assign(root: string, group: string): void {
    this.write(assign(this.groups, root, group));
  }

  collapse(id: string): void {
    const next = new Set(this.collapsed);
    if (!next.delete(id)) next.add(id);
    this.collapsed = next;
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
