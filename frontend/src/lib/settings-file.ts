import { readSettings, writeSetting } from "$lib/ipc";

type Document = Record<string, unknown>;

/** Read once per window: three unrelated modules want three keys of the same document,
    and each of them asking separately was three round trips for one file. */
let document: Promise<Document | null> | null = null;
/** Keys whose reader got nothing because the file could not be read (a scanner holding it):
    what it holds now are defaults, and writing them would replace the user's own. */
const unread = new Set<string>();

async function load(): Promise<Document | null> {
  document ??= readSettings()
    .then((text) => {
      const parsed: unknown = JSON.parse(text);
      return parsed && typeof parsed === "object" ? (parsed as Document) : {};
    })
    .catch(() => {
      document = null;
      return null;
    });
  return document;
}

/** Another window wrote the file: the next read goes to disk. */
export function forgetSettings(): void {
  document = null;
}

export async function readKey<T>(key: string): Promise<T | undefined> {
  const read = await load();
  if (read === null) unread.add(key);
  else unread.delete(key);
  return read?.[key] as T | undefined;
}

export async function writeKey(key: string, value: unknown): Promise<void> {
  if (unread.has(key)) throw new Error("settings.json could not be read, so it is not written over");
  const read = await load();
  if (read) read[key] = value;
  await writeSetting(key, JSON.stringify(value));
}
