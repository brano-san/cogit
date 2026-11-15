import { isOn, NETWORK_FIELDS, type SettingField } from "./repo-settings";

/** Preferences ▸ Network (F-942): the user's global `http.proxy`, `http.sslVerify` and
    `http.sslCAInfo`, written with `git config --global` the moment they change. */

export { NETWORK_FIELDS };

/** What a field holds now, from what was read: `null` is not set. */
export type GlobalValues = ReadonlyMap<string, string | null>;

/** What to write for what the user typed or ticked; `null` removes the key. A tick on
    TLS checking is git's default, so it removes `sslVerify = false` rather than writing true. */
export function toWrite(field: SettingField, input: string | boolean): string | null {
  if (field.control.kind === "bool") {
    const on = input === true || (typeof input === "string" && isOn(input, field.control));
    return on ? null : field.control.off;
  }
  const text = String(input).trim();
  return text === "" ? null : text;
}

/** The tick a bool field shows: git's default when the key is not set. */
export function ticked(field: SettingField, value: string | null): boolean {
  if (field.control.kind !== "bool") return false;
  return isOn(value ?? field.fallback, field.control);
}

/** Whether writing `next` changes anything; a field is written only when it does. */
export function changes(values: GlobalValues, key: string, next: string | null): boolean {
  return (values.get(key) ?? null) !== next;
}
