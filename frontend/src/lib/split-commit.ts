import type { SplitFile, SplitPart, SplitPreview } from "./ipc";

export interface SplitBlockRef {
  key: string;
  path: string;
  hunk: number | null;
}

export interface SplitDraft {
  messages: string[];
  owners: Record<string, number>;
  active: number;
}

export function blockKey(path: string, hunk: number | null): string {
  return hunk === null ? `${path}\u0000*` : `${path}\u0000${hunk}`;
}

export function blocksOf(files: readonly SplitFile[]): SplitBlockRef[] {
  return files.flatMap((file): SplitBlockRef[] =>
    file.whole
      ? [{ key: blockKey(file.path, null), path: file.path, hunk: null }]
      : file.blocks.map((_, hunk) => ({ key: blockKey(file.path, hunk), path: file.path, hunk })),
  );
}

export function newDraft(message: string): SplitDraft {
  return { messages: [message, message], owners: {}, active: 0 };
}

export function ownerOf(draft: SplitDraft, key: string): number {
  return draft.owners[key] ?? draft.messages.length - 1;
}

export function isAssigned(draft: SplitDraft, key: string): boolean {
  return draft.owners[key] !== undefined;
}

export function assign(draft: SplitDraft, keys: readonly string[], part: number): SplitDraft {
  if (keys.length === 0 || part < 0 || part >= draft.messages.length) return draft;
  const owners = { ...draft.owners };
  const already = keys.every((key) => owners[key] === part);
  for (const key of keys) {
    if (already) delete owners[key];
    else owners[key] = part;
  }
  return { ...draft, owners };
}

export function setActive(draft: SplitDraft, part: number): SplitDraft {
  if (part < 0 || part >= draft.messages.length) return draft;
  return { ...draft, active: part };
}

export function setMessage(draft: SplitDraft, part: number, message: string): SplitDraft {
  if (part < 0 || part >= draft.messages.length) return draft;
  const messages = draft.messages.slice();
  messages[part] = message;
  return { ...draft, messages };
}

export function addPart(draft: SplitDraft, message: string): SplitDraft {
  const messages = [...draft.messages, message];
  return { ...draft, messages, active: messages.length - 1 };
}

export function removePart(draft: SplitDraft, part: number): SplitDraft {
  if (draft.messages.length <= 2 || part < 0 || part >= draft.messages.length) return draft;
  const messages = draft.messages.filter((_, at) => at !== part);
  const owners: Record<string, number> = {};
  for (const [key, owner] of Object.entries(draft.owners)) {
    if (owner === part) continue;
    owners[key] = owner > part ? owner - 1 : owner;
  }
  const active = Math.min(draft.active > part ? draft.active - 1 : draft.active, messages.length - 1);
  return { messages, owners, active };
}

export function partSizes(draft: SplitDraft, blocks: readonly SplitBlockRef[]): number[] {
  const sizes = draft.messages.map(() => 0);
  for (const block of blocks) {
    const owner = ownerOf(draft, block.key);
    sizes[owner] = (sizes[owner] ?? 0) + 1;
  }
  return sizes;
}

export function splitCommitProblem(draft: SplitDraft, blocks: readonly SplitBlockRef[]): string | null {
  if (blocks.length === 0) return "The commit has no blocks to split.";
  if (blocks.length < 2) return "The commit has a single block: there is nothing to split.";
  const sizes = partSizes(draft, blocks);
  const empty = sizes.findIndex((size) => size === 0);
  if (empty >= 0) {
    return empty === sizes.length - 1
      ? `Leave at least one block for commit ${empty + 1}.`
      : `Give commit ${empty + 1} at least one block.`;
  }
  const silent = draft.messages.findIndex((message) => message.trim() === "");
  if (silent >= 0) return `Commit ${silent + 1} needs a message.`;
  return null;
}

export function splitParts(draft: SplitDraft, blocks: readonly SplitBlockRef[]): SplitPart[] {
  const parts: SplitPart[] = draft.messages.map((message) => ({ message, blocks: [] }));
  for (const block of blocks) {
    parts[ownerOf(draft, block.key)]?.blocks.push({ path: block.path, hunk: block.hunk });
  }
  return parts;
}

export function draftStarted(draft: SplitDraft, original: string): boolean {
  return (
    Object.keys(draft.owners).length > 0 ||
    draft.messages.length > 2 ||
    draft.messages.some((message) => message !== original)
  );
}

export interface SplitCommitRequest {
  oid: string;
  message: string;
  files: SplitFile[];
  published: boolean;
}

export async function splitCommitRequest(
  oid: string,
  read: {
    preview: (oid: string) => Promise<SplitPreview>;
    published: (oid: string) => Promise<boolean>;
  },
): Promise<SplitCommitRequest> {
  const [preview, published] = await Promise.all([read.preview(oid), read.published(oid)]);
  return { oid, ...preview, published };
}
