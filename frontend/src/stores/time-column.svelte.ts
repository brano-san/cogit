import { measuredTimeWidth, timeWidth, type GraphTimeFormat } from "$lib/graph-row";

/** The graph's date column: as wide as the widest date the format writes, measured in the
    column's own font, so nothing is reserved past it. Measured again when the fonts load. */
class TimeColumn {
  #fonts = $state(0);
  #cache = new Map<string, number>();
  #canvas: CanvasRenderingContext2D | null = null;

  constructor() {
    if (typeof document !== "undefined") {
      document.fonts?.addEventListener("loadingdone", () => {
        this.#cache.clear();
        this.#fonts += 1;
      });
    }
  }

  /** CSS pixels of text, without the padding the cell adds from tokens. */
  width(format: GraphTimeFormat): number {
    void this.#fonts;
    const font = this.#font();
    if (!font) return timeWidth(format);
    const key = `${format}|${font}`;
    const known = this.#cache.get(key);
    if (known !== undefined) return known;
    const context = (this.#canvas ??= document.createElement("canvas").getContext("2d"));
    if (!context) return timeWidth(format);
    context.font = font;
    const width = measuredTimeWidth(format, Math.floor(Date.now() / 1000), (text) => context.measureText(text).width);
    this.#cache.set(key, width);
    return width;
  }

  /** The column's font: the header size in the UI family, as `.date` in CommitRow sets it. */
  #font(): string | null {
    if (typeof document === "undefined" || typeof getComputedStyle === "undefined") return null;
    const root = getComputedStyle(document.documentElement);
    const size = root.getPropertyValue("--fs-header").trim();
    const family = root.getPropertyValue("--font-ui").trim();
    return size && family ? `${size} ${family}` : null;
  }
}

export const timeColumn = new TimeColumn();
