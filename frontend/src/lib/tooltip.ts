/** One delay for every tooltip in the app, native `title` ones included (R-181). */
export const TIP_DELAY_MS = 400;

const GAP = 6;
const MARGIN = 4;

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface Placement {
  left: number;
  top: number;
  below: boolean;
}

export function placeTip(
  anchor: Box,
  tip: { width: number; height: number },
  viewport: { width: number; height: number },
  preferBelow = false,
): Placement {
  const roomAbove = anchor.top - GAP - tip.height >= MARGIN;
  const roomBelow = anchor.top + anchor.height + GAP + tip.height <= viewport.height - MARGIN;
  const below = preferBelow ? roomBelow || !roomAbove : !roomAbove && roomBelow;

  const centred = anchor.left + anchor.width / 2 - tip.width / 2;
  const left = Math.max(MARGIN, Math.min(centred, viewport.width - MARGIN - tip.width));
  const top = below ? anchor.top + anchor.height + GAP : anchor.top - GAP - tip.height;
  return { left, top: Math.max(MARGIN, top), below };
}

/** Where a title waits for the app's tooltip, out of the browser's reach. */
export const STASH = "data-cogit-title";

interface Attributes {
  getAttribute(name: string): string | null;
  setAttribute(name: string, value: string): void;
  removeAttribute(name: string): void;
}

/** Leaves `title=""` in place of the title, not nothing: removing an attribute that is not
    there records no mutation, so the owner's later removal would go unseen. `before` is the
    title the change replaced: `""` to `""` is the owner emptying it, not this placeholder. */
export function stashTitle(element: Attributes, before?: string | null): void {
  const title = element.getAttribute("title");
  if (title === null || (title === "" && before === "")) element.removeAttribute(STASH);
  else if (title !== "") {
    element.setAttribute(STASH, title);
    element.setAttribute("title", "");
  }
}

/** Every title in `root` goes aside as soon as it is there. Chromium reads a title for its
    native tooltip before the page sees the pointer arrive, so moving it aside on hover was
    too late: a pointer that stopped right after entering got both tooltips. */
export function watchTitles(root: Document): () => void {
  const sweep = (node: Node) => {
    if (!(node instanceof Element)) return;
    if (node.hasAttribute("title")) stashTitle(node);
    for (const titled of node.querySelectorAll("[title]")) stashTitle(titled);
  };
  const observer = new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === "attributes") stashTitle(record.target as Element, record.oldValue);
      else record.addedNodes.forEach(sweep);
    }
  });
  observer.observe(root, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeOldValue: true,
    attributeFilter: ["title"],
  });
  sweep(root.documentElement);
  return () => observer.disconnect();
}
