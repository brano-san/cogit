export interface TextPart {
  text: string;
  href: string | null;
}

/** Trailing punctuation is excluded: `https://x.test.` ends a sentence, not a host. */
const URL_PATTERN = /https?:\/\/[^\s<>"']*[^\s<>"'.,;:!?)\]}]/g;

export function splitLinks(text: string): TextPart[] {
  const parts: TextPart[] = [];
  let at = 0;

  for (const match of text.matchAll(URL_PATTERN)) {
    const start = match.index;
    if (start > at) parts.push({ text: text.slice(at, start), href: null });
    parts.push({ text: match[0], href: match[0] });
    at = start + match[0].length;
  }
  if (at < text.length) parts.push({ text: text.slice(at), href: null });
  return parts;
}
