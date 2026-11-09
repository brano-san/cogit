export const CHILD_WINDOWS = ["blame", "commit", "compare", "editor", "errors", "investigate", "solver"] as const;

export type ChildWindow = (typeof CHILD_WINDOWS)[number];

export function windowOf(address: string): ChildWindow | null {
  const page = /^\/([a-z]+)\.html(?:[?#]|$)/.exec(address)?.[1];
  return CHILD_WINDOWS.find((kind) => kind === page) ?? null;
}
