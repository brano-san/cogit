/** Every "Don't show again" in one list, so each can be taken back (requirement 1.6). */
export interface SuppressedChoice {
  id: string;
  label: string;
  scope: string | null;
}

export const CONFIRM_EXIT = "confirm-exit";
export const CONFIRM_LOCAL_CHECKOUT = "confirm-local-checkout";

/** root → warning id → title. */
export type IgnoredWarnings = Record<string, Record<string, string>>;

export function healthChoiceId(root: string, warning: string): string {
  return `health\n${root}\n${warning}`;
}

export type ParsedChoice =
  | { kind: "confirmExit" }
  | { kind: "confirmLocalCheckout" }
  | { kind: "health"; root: string; warning: string }
  | { kind: "unknown" };

export function parseChoice(id: string): ParsedChoice {
  if (id === CONFIRM_EXIT) return { kind: "confirmExit" };
  if (id === CONFIRM_LOCAL_CHECKOUT) return { kind: "confirmLocalCheckout" };
  const [kind, root, warning] = id.split("\n");
  if (kind === "health" && root !== undefined && warning !== undefined) {
    return { kind: "health", root, warning };
  }
  return { kind: "unknown" };
}

function nameOf(root: string): string {
  return root.split("/").filter(Boolean).at(-1) ?? root;
}

export function suppressedChoices(
  confirmExit: boolean,
  ignored: IgnoredWarnings,
  confirmLocalCheckout = true,
): SuppressedChoice[] {
  const choices: SuppressedChoice[] = [];
  if (!confirmExit) choices.push({ id: CONFIRM_EXIT, label: "Confirm before exiting", scope: null });
  if (!confirmLocalCheckout) {
    choices.push({ id: CONFIRM_LOCAL_CHECKOUT, label: "Check Out dialog for a local branch", scope: null });
  }
  for (const root of Object.keys(ignored).sort()) {
    for (const [warning, title] of Object.entries(ignored[root] ?? {})) {
      choices.push({ id: healthChoiceId(root, warning), label: title, scope: nameOf(root) });
    }
  }
  return choices;
}
