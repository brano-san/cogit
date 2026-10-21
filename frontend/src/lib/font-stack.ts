import type { Os } from "./platform";

/** Desktop UI families after the desktop's own, where Linux has them. */
const UI_FALLBACK = ["Noto Sans", "Cantarell", "Ubuntu", "DejaVu Sans", "Liberation Sans"];

/** The families Linux is asked about (`fc-list`), so the stack names only installed ones. */
export const STACK_CANDIDATES: readonly string[] = UI_FALLBACK;

export interface DesktopFont {
  family: string | null;
  installed: readonly string[] | null;
}

const quoted = (family: string) => (/^[\w-]+$/.test(family) ? family : `"${family.replace(/"/g, "")}"`);
const stack = (families: readonly string[], generic: string) =>
  [...new Set(families)].map(quoted).concat(generic).join(", ");

/** The one font of the whole app, the diff and code included (R-778): the family chosen in
    Preferences, then the desktop's UI font (SmartGit takes it too), then only families the
    system has — Windows and macOS ship theirs, Linux is asked. */
export function fontStack(os: Os, font: DesktopFont | null, chosen = ""): string {
  const own = chosen.trim() ? [chosen.trim()] : [];
  if (os === "windows") return stack([...own, font?.family ?? "Segoe UI"], "sans-serif");
  if (os === "mac") return [...own.map(quoted), "-apple-system", "system-ui", "sans-serif"].join(", ");
  const has = (family: string) => font?.installed?.includes(family) ?? true;
  const desktop = font?.family ? [font.family] : [];
  return stack([...own, ...desktop, ...UI_FALLBACK.filter(has)], "sans-serif");
}
