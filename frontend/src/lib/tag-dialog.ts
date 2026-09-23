import type { TagRequest } from "./ipc";

export function tagNameHint(name: string, taken: readonly string[]): string | null {
  const trimmed = name.trim();
  if (trimmed === "") return "Enter a name.";
  if (taken.includes(trimmed)) return `A tag named '${trimmed}' already exists.`;
  return null;
}

export function tagRequest(name: string, message: string, target: string): TagRequest {
  const text = message.replace(/\s+$/, "");
  return { name: name.trim(), target, message: text.trim() === "" ? null : text, force: false };
}
