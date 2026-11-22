export type GlyphGrid = 8 | 16 | 24;

export interface Glyph {
  readonly grid: GlyphGrid;
  readonly d: string;
  readonly line?: string;
}

function ring(cx: number, cy: number, r: number): string {
  return `M${cx - r} ${cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0Z`;
}

function disk(x: number, y: number, width: number): string {
  return `M${x + 1} ${y}h${width - 2}a1 1 0 0 1 1 1v1a1 1 0 0 1-1 1h${2 - width}a1 1 0 0 1-1-1v-1a1 1 0 0 1 1-1Z`;
}

const g8 = (d: string): Glyph => ({ grid: 8, d });
const g16 = (d: string, line?: string): Glyph => (line === undefined ? { grid: 16, d } : { grid: 16, d, line });
const g24 = (d: string): Glyph => ({ grid: 24, d });

const PAGE = "M3.5 1.5H9.5L12.5 4.5V14.5H3.5Z";
const PAGE_FOLD = "M9.5 1.5V4.5H12.5";
const FOLDER = "M1.5 4A1.5 1.5 0 0 1 3 2.5h3l1.5 1.5H13A1.5 1.5 0 0 1 14.5 5.5V12A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12Z";
const DISKS = [1.5, 6.5, 11.5].map((y) => disk(1.5, y, 13)).join("");

export const GLYPHS = {
  close: g16("M4 4l8 8M12 4l-8 8"),
  check: g16("M3 8.5 6.5 12 13 4.5"),
  generateMessage: g16("M7 2c0 3.3 2.7 6 6 6-3.3 0-6 2.7-6 6 0-3.3-2.7-6-6-6 3.3 0 6-2.7 6-6ZM13 2v4m-2-2h4"),
  warning: g16("M8 2 14.5 13.5h-13ZM8 6.5v3.5M8 11.8v.2"),
  error: g16(`${ring(8, 8, 6.5)}M8 4.5v4M8 11v.5`),
  info: g16(`${ring(8, 8, 6.5)}M8 7.5v4M8 5v.5`),
  more: g24("M5 12h.01M12 12h.01M19 12h.01"),

  file: g16(PAGE, PAGE_FOLD),
  symlink: g16(PAGE, "M6 12V9.5A1.5 1.5 0 0 1 7.5 8H10M8.5 6.5 10 8 8.5 9.5"),
  folder: g16(FOLDER),
  folderPlus: g16(FOLDER, "M8 6.5v5M5.5 9h5"),
  repository: g16(DISKS),
  scanRepositories: g16(
    `${disk(1.5, 1.5, 9)}${disk(1.5, 6.5, 5)}${disk(1.5, 11.5, 5)}`,
    `${ring(11, 10.5, 3)}M13.2 12.7 15 14.5`,
  ),
  worktree: g16(`${ring(4.5, 3.5, 1.5)}${ring(4.5, 12.5, 1.5)}${ring(11.5, 5.5, 1.5)}M4.5 5v6M11.5 7c0 2.5-3 3-7 4`),
  lock: g16("M4.5 7.5h7a1 1 0 0 1 1 1V13a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1V8.5a1 1 0 0 1 1-1ZM5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5"),
  skipped: g16(PAGE, `${PAGE_FOLD}M5.5 8.5l2 2-2 2M8.5 8.5l2 2-2 2`),

  prev: g16("m3 10 5-5 5 5"),
  next: g16("m3 6 5 5 5-5"),
  prevConflict: g16("M3 2.5h10M4 11l4-4 4 4"),
  nextConflict: g16("M3 13.5h10M4 5l4 4 4-4"),
  back: g16("M13.5 8h-11M6.5 4 2.5 8l4 4"),
  forward: g16("M2.5 8h11M9.5 4l4 4-4 4"),
  moveUp: g16("M8 2.5 13 8h-3v5.5H6V8H3Z"),
  moveDown: g16("M8 13.5 3 8h3V2.5h4V8h3Z"),
  unfoldUp: g16("M8 11V3.5M5 6.5l3-3 3 3M3 13.5h10"),
  unfoldDown: g16("M8 5v7.5M5 9.5l3 3 3-3M3 2.5h10"),
  ahead: g8("M4 7V1.5M1.5 4 4 1.5 6.5 4"),
  behind: g8("M4 1v5.5M1.5 4 4 6.5 6.5 4"),
  clone: g16("M8 2v8M4.5 6.5 8 10l3.5-3.5M2.5 11v1.5A1 1 0 0 0 3.5 13.5h9a1 1 0 0 0 1-1V11"),
  bookmark: g24("m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"),

  pull: g24("M12 3v12m0 0 4-4m-4 4-4-4M5 21h14"),
  push: g24("M12 21V9m0 0 4 4m-4-4-4 4M5 3h14"),
  sync: g24("M21 12a9 9 0 0 1-9 9 9 9 0 0 1-8.5-6M3 12a9 9 0 0 1 9-9 9 9 0 0 1 8.5 6M21 4v5h-5M3 20v-5h5"),
  stage: g24("M12 5v14m-7-7h14"),
  unstage: g24("M5 12h14"),
  discard: g24("M3 12a9 9 0 1 0 3-6.7L3 8m0-5v5h5"),
  undo: g24("M9 14 4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5 5.5 5.5 0 0 1-5.5 5.5H11"),
  stash: g24("M3 8h18M3 8l2-4h14l2 4M3 8v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8m-11 5h4"),
  applyStash: g24("M3 8h18M3 8l2-4h14l2 4M3 8v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8M12 17v-6m-3 3 3-3 3 3"),
  merge: g24(
    "M7 18V9a4 4 0 0 1 4-4h5M7 6.5a2.5 2.5 0 1 0 0-.1M18.5 7.5a2.5 2.5 0 1 0 0-.1M7 20.5a2.5 2.5 0 1 0 0-.1",
  ),
  rebase: g24(
    "M7 6.5a2.5 2.5 0 1 0 0-.1M7 20.5a2.5 2.5 0 1 0 0-.1M7 9v6M17 6.5a2.5 2.5 0 1 0 0-.1M17 9v4a4 4 0 0 1-4 4H9",
  ),
  tag: g24("M3 11V5a2 2 0 0 1 2-2h6l10 10-8 8L3 11Zm4-4.5a.5.5 0 1 0 0-.1"),

  splitIndex: g24("M4 4h10v10H4zM10 10h10v10H10m2-2 2 2 4-4"),
  directories: g24(
    "M3 6.5A1.5 1.5 0 0 1 4.5 5h3l1.5 2h10a1.5 1.5 0 0 1 1.5 1.5v9A1.5 1.5 0 0 1 19 19H4.5A1.5 1.5 0 0 1 3 17.5ZM9 11v5m0-5h4m-4 5h4",
  ),
  flatList: g24("M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"),
  columns: g24("M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2ZM9 3v18M15 3v18"),
  expandAll: g24("m7 15 5 5 5-5M7 9l5-5 5 5"),
  collapseAll: g24("m7 20 5-5 5 5M7 4l5 5 5-5"),

  base: g16("M8 14V8M8 8 4 3M8 8l4-5M2.5 3h3M10.5 3h3"),
  save: g16("M3 2.5h8l2.5 2.5v8.5h-10.5zM5.5 2.5v3h5v-3M5.5 13.5v-4h5v4"),
  takeOursTheirs: g16("M2 4.5h9M8 2l3 2.5L8 7M2 11.5h9M8 9l3 2.5L8 14M14 2v12"),
  takeOurs: g16("M2 8h10M8.5 4.5 12 8l-3.5 3.5M14 3v10"),
  takeTheirs: g16("M14 8H4M7.5 4.5 4 8l3.5 3.5M2 3v10"),
  takeTheirsOurs: g16("M14 4.5H5M8 2 5 4.5 8 7M14 11.5H5M8 9l-3 2.5L8 14M2 2v12"),
  keepOurs: g16("M7 2h4l3 3v9H7zM11 2v3h3M1.5 9.5H5M3.5 7.5l2 2-2 2"),
  keepTheirs: g16("M2 2h4l3 3v9H2zM6 2v3h3M14.5 9.5H11M12.5 7.5l-2 2 2 2"),
  layoutOursResult: g16("M2 3h12v10H2zM8 3v10M3.5 5.5h3M3.5 8h3"),
  layoutAll: g16("M2 3h12v10H2zM6 3v10M10 3v10"),
  layoutResultTheirs: g16("M2 3h12v10H2zM8 3v10M9.5 5.5h3M9.5 8h3"),
  layoutResultBelow: g16("M2 3h12v10H2zM2 8h12M8 3v5"),
  delete: g16("M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 9h5.6l.7-9M7 7v4.5M9 7v4.5"),
  external: g16("M9 2.5h4.5V7M13.5 2.5 8 8M11.5 9.5v4h-9v-9h4"),
} as const satisfies Record<string, Glyph>;

export type GlyphName = keyof typeof GLYPHS;

export const STROKE: Record<GlyphGrid, number> = { 8: 1, 16: 1.5, 24: 2 };
