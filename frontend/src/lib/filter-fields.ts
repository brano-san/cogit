import type { TextFields } from "$lib/ipc/bindings";

/** The switches under the graph filter, in the order they show: where the typed text is
    looked for (F-560). */
export const FILTER_FIELDS = ["author", "committer", "message", "refs", "id", "name", "content"] as const;
export type FilterField = (typeof FILTER_FIELDS)[number];

/** All but the two that read trees and files, which are slow on a long history. */
export const DEFAULT_FILTER_FIELDS: readonly FilterField[] = ["author", "committer", "message", "refs", "id"];

export const FIELD_LABELS: Record<FilterField, { label: string; title: string }> = {
  author: { label: "Author", title: "The author's name or email" },
  committer: { label: "Committer", title: "The committer's name or email" },
  message: { label: "Message", title: "The whole commit message, body included" },
  refs: { label: "Refs", title: "Names of branches and tags at the commit" },
  id: { label: "ID", title: "The start of the commit id" },
  name: {
    label: "Name",
    title: "Names of the files the commit changed; with a / in the text, their whole paths. Slower.",
  },
  content: {
    label: "Content",
    title: "Lines the commit added or removed. Reads every changed file: the slowest.",
  },
};

/** Known fields only, each once, in the switches' order: a field a newer version added
    does not wipe the rest. */
export function knownFields(value: unknown): FilterField[] | undefined {
  if (!Array.isArray(value)) return undefined;
  return FILTER_FIELDS.filter((field) => value.includes(field));
}

export function textFields(on: readonly FilterField[]): TextFields {
  const fields = Object.fromEntries(FILTER_FIELDS.map((field) => [field, on.includes(field)]));
  return fields as Required<TextFields>;
}

export function toggled(on: readonly FilterField[], field: FilterField): FilterField[] {
  return FILTER_FIELDS.filter((each) => (each === field ? !on.includes(each) : on.includes(each)));
}
