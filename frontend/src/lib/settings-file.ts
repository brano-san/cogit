import { readSettings, writeSetting } from "$lib/ipc";

type Document = Record<string, unknown>;

/** Read once per window: three unrelated modules want three keys of the same document,
    and each of them asking separately was three round trips for one file. */
let document: Promise<Document> | null = null;

async function load(): Promise<Document> {
  document ??= readSettings()
    .then((text) => {
      const parsed: unknown = JSON.parse(text);
      return parsed && typeof parsed === "object" ? (parsed as Document) : {};
    })
    .catch(() => {
      document = null;
      return {};
    });
  return document;
}

export async function readKey<T>(key: string): Promise<T | undefined> {
  return (await load())[key] as T | undefined;
}

export async function writeKey(key: string, value: unknown): Promise<void> {
  (await load())[key] = value;
  await writeSetting(key, JSON.stringify(value));
}

/** Tests only: the cache outlives a component, so it has to be emptied between cases. */
export function forgetSettings(): void {
  document = null;
}
