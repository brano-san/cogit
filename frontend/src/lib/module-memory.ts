/** Which submodule trees of the Repositories list are open, kept across restarts (R-352):
    the repositories whose top level shows, and inside each the nodes expanded, by key
    from the top. Keyed by root, as the groups and the list are. */
export interface ModuleMemory {
  open: string[];
  nodes: Record<string, string[]>;
}

export const EMPTY_MEMORY: ModuleMemory = { open: [], nodes: {} };

function strings(value: unknown): string[] {
  return Array.isArray(value)
    ? [...new Set(value.filter((item): item is string => typeof item === "string"))]
    : [];
}

export function readModuleMemory(raw: unknown): ModuleMemory {
  if (typeof raw !== "object" || raw === null) return { open: [], nodes: {} };
  const record = raw as Record<string, unknown>;
  const nodes: Record<string, string[]> = {};
  if (typeof record.nodes === "object" && record.nodes !== null) {
    for (const [root, keys] of Object.entries(record.nodes)) {
      const kept = strings(keys);
      if (kept.length > 0) nodes[root] = kept;
    }
  }
  return { open: strings(record.open), nodes };
}

export function withTop(memory: ModuleMemory, root: string, open: boolean): ModuleMemory {
  if (memory.open.includes(root) === open) return memory;
  return {
    ...memory,
    open: open ? [...memory.open, root] : memory.open.filter((each) => each !== root),
  };
}

export function withNode(
  memory: ModuleMemory,
  root: string,
  key: string,
  open: boolean,
): ModuleMemory {
  const keys = memory.nodes[root] ?? [];
  if (keys.includes(key) === open) return memory;
  const next = open ? [...keys, key] : keys.filter((each) => each !== key);
  const nodes = { ...memory.nodes };
  if (next.length > 0) nodes[root] = next;
  else delete nodes[root];
  return { ...memory, nodes };
}

/** Removing a repository from the list forgets its tree; closing it does not. */
export function forgetRoot(memory: ModuleMemory, root: string): ModuleMemory {
  if (!memory.open.includes(root) && !(root in memory.nodes)) return memory;
  const nodes = { ...memory.nodes };
  delete nodes[root];
  return { open: memory.open.filter((each) => each !== root), nodes };
}

/** A repository just opened shows its submodules at once, one level: the nodes below keep
    what was remembered of them (R-546). */
export function withOpenedRepository(memory: ModuleMemory, root: string): ModuleMemory {
  return withTop(memory, root, true);
}
