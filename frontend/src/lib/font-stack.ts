import type { Os } from "./platform";

/** Desktop UI families after the desktop's own, where Linux has them. */
const UI_FALLBACK = ["Noto Sans", "Cantarell", "Ubuntu", "DejaVu Sans", "Liberation Sans"];
const MONO_FALLBACK = [
  "JetBrains Mono",
  "Cascadia Mono",
  "Fira Code",
  "Ubuntu Mono",
  "Noto Sans Mono",
  "DejaVu Sans Mono",
  "Liberation Mono",
];

/** The families Linux is asked about (`fc-list`), so the stacks name only installed ones. */
export const STACK_CANDIDATES: readonly string[] = [...UI_FALLBACK, ...MONO_FALLBACK];

export interface DesktopFont {
  family: string | null;
  installed: readonly string[] | null;
}

const quoted = (family: string) => (/^[\w-]+$/.test(family) ? family : `"${family.replace(/"/g, "")}"`);
const stack = (families: readonly string[], generic: string) =>
  [...new Set(families)].map(quoted).concat(generic).join(", ");

/** The UI and code font stacks: the desktop's UI font first (SmartGit takes it too), then only
    families the system has — Windows and macOS ship theirs, Linux is asked. */
export function fontStacks(os: Os, font: DesktopFont | null): { ui: string; mono: string } {
  if (os === "windows") return { ui: stack([font?.family ?? "Segoe UI"], "sans-serif"), mono: stack(["Consolas"], "monospace") };
  if (os === "mac") return { ui: "-apple-system, system-ui, sans-serif", mono: '"SF Mono", Menlo, monospace' };
  const has = (family: string) => font?.installed?.includes(family) ?? true;
  const desktop = font?.family ? [font.family] : [];
  return {
    ui: stack([...desktop, ...UI_FALLBACK.filter(has)], "sans-serif"),
    mono: stack(MONO_FALLBACK.filter(has), "monospace"),
  };
}
