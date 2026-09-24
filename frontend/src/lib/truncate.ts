/** The app's one truncation rule (#5, doc/12-risks.md, R-243). Lists cut on the right, in
    CSS: `.truncate` on the text, `.shrink-first` on what a row gives up first (a URL, a
    path, a subject) and `.shrink-last` on its name. Only what has to keep both ends is cut
    in the middle, here. */
const ELLIPSIS = "…";

/** How long a ref label in the graph may be before its middle goes. */
export const REF_LABEL_MAX = 30;

function charCut(text: string, max: number): string {
  if (max <= 0) return "";
  const room = max - 1;
  const head = Math.floor(room / 2);
  const tail = room - head;
  return text.slice(0, head) + ELLIPSIS + (tail > 0 ? text.slice(-tail) : "");
}

/** `feature/14340…new_toolchain`: the start and the end that tells branches apart. */
export function truncateMiddle(text: string, max: number): string {
  return text.length <= max ? text : charCut(text, max);
}

export function refLabelText(text: string): string {
  return truncateMiddle(text, REF_LABEL_MAX);
}

/** The tail a squeezed ref label keeps whole; the lead gives way with an ellipsis in CSS,
    which puts the cut in the middle at whatever width the row leaves it (#12). */
const LABEL_TAIL = 10;

export function middleCut(text: string): { lead: string; tail: string } {
  const tail = Math.min(LABEL_TAIL, Math.floor(text.length / 2));
  return { lead: text.slice(0, text.length - tail), tail: text.slice(text.length - tail) };
}

/** `C:\Users\brano\…\logs\cogit.log`: whole folders dropped from the middle to fit `max`. */
export function truncatePath(path: string, max: number): string {
  if (path.length <= max) return path;
  const sep = path.includes("\\") ? "\\" : "/";
  const root = path.match(sep === "\\" ? /^\\+/ : /^\/+/)?.[0] ?? "";
  const parts = path.slice(root.length).split(sep);

  const head: string[] = [];
  const tail: string[] = [parts[parts.length - 1]!];
  const render = () =>
    root + (head.length > 0 ? head.join(sep) + sep : "") + ELLIPSIS + sep + tail.join(sep);
  if (render().length > max) return charCut(path, max);

  let growing = true;
  while (growing) {
    growing = false;
    for (const side of ["tail", "head"] as const) {
      if (head.length + tail.length >= parts.length - 1) return render();
      if (side === "tail") tail.unshift(parts[parts.length - 1 - tail.length]!);
      else head.push(parts[head.length]!);
      if (render().length <= max) {
        growing = true;
      } else if (side === "tail") {
        tail.shift();
      } else {
        head.pop();
      }
    }
  }
  return render();
}

function charWidth(node: HTMLElement): number {
  const context = document.createElement("canvas").getContext("2d");
  if (!context) return 0;
  context.font = getComputedStyle(node).font;
  // Only meaningful in a monospace font, where every glyph is this wide.
  return context.measureText("M").width;
}

/** Svelte action: shows `path` cut in the middle to whatever width the node is given. */
export function fitPath(node: HTMLElement, path: string) {
  let current = path;
  const fit = () => {
    const width = node.clientWidth;
    const glyph = charWidth(node);
    node.textContent =
      width > 0 && glyph > 0 ? truncatePath(current, Math.floor(width / glyph)) : current;
  };
  const observer = new ResizeObserver(fit);
  observer.observe(node);
  fit();
  return {
    update(next: string) {
      current = next;
      fit();
    },
    destroy() {
      observer.disconnect();
    },
  };
}
