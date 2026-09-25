/** A letter key by where it sits, anything else as the layout reports it, lower case. With
    a Russian layout Ctrl+F types «а», and it is still Ctrl+F. */
export function keyLetter(event: { key: string; code?: string }): string {
  const place = /^Key([A-Z])$/.exec(event.code ?? "");
  return (place?.[1] ?? event.key).toLowerCase();
}
