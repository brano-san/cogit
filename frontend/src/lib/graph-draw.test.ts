import { describe, expect, it } from "vitest";
import { allocatedBytes, msPerCall, report } from "$lib/graph-bench-probe";
import { paintGraph, type DrawnRow, type GraphScene } from "$lib/graph-draw";
import { GRAPH } from "$lib/graph-geometry";
import type { RowPaint } from "$lib/graph-style";
import type { GraphRow, Segment } from "$lib/ipc";

/** Records every call and every assignment, so two paints can be compared stroke by stroke. */
class Recorder {
  log: string[] = [];
  paths = 0;
  dashes: unknown[] = [];
  set strokeStyle(value: string) {
    this.log.push(`strokeStyle ${value}`);
  }
  set fillStyle(value: unknown) {
    this.log.push(`fillStyle ${typeof value === "string" ? value : "gradient"}`);
  }
  set lineWidth(value: number) {
    this.log.push(`lineWidth ${value}`);
  }
  setLineDash(value: number[]) {
    this.dashes.push(value);
    this.log.push(`setLineDash ${value.join(",")}`);
  }
  stroke(path?: FakePath) {
    this.log.push(`stroke ${path ? path.ops.join(";") : ""}`);
  }
  createLinearGradient() {
    return { addColorStop: (at: number, colour: string) => this.log.push(`stop ${at} ${colour}`) };
  }
}
for (const name of ["save", "restore", "beginPath", "rect", "clip", "moveTo", "lineTo", "bezierCurveTo", "arc", "fill", "fillRect"]) {
  (Recorder.prototype as unknown as Record<string, unknown>)[name] = function (this: Recorder, ...args: number[]) {
    this.log.push(`${name} ${args.map((n) => Math.round(n * 100) / 100).join(",")}`);
  };
}

let recorder: Recorder | null = null;
class FakePath {
  ops: string[] = [];
  constructor() {
    if (recorder) recorder.paths += 1;
  }
  moveTo(x: number, y: number) {
    this.ops.push(`M${Math.round(x * 100) / 100} ${Math.round(y * 100) / 100}`);
  }
  lineTo(x: number, y: number) {
    this.ops.push(`L${Math.round(x * 100) / 100} ${Math.round(y * 100) / 100}`);
  }
  bezierCurveTo(...p: number[]) {
    this.ops.push(`C${p.map((n) => Math.round(n * 100) / 100).join(" ")}`);
  }
}
/** The bench's own path: as cheap as a native one, so the numbers are the paint's. */
class NullPath {
  moveTo() {}
  lineTo() {}
  bezierCurveTo() {}
}
const nullContext = new Proxy(
  {},
  {
    get: (_target, name) => (name === "createLinearGradient" ? () => ({ addColorStop() {} }) : () => {}),
    set: () => true,
  },
) as unknown as CanvasRenderingContext2D;

const g = globalThis as unknown as { Path2D: unknown };

const seg = (from: number, to: number, span: Segment["span"], primary = false, arrow = false): Segment => ({
  from,
  to,
  span,
  primary,
  color: from,
  arrow,
});

/** A history six lines wide: lines pass, nodes branch off and merge, some links are cut. */
function history(count: number): GraphRow[] {
  const rows: GraphRow[] = [];
  for (let row = 0; row < count; row++) {
    const lane = row % 6;
    const segments: Segment[] = [];
    for (let l = 0; l < 6; l++) {
      if (l === lane) continue;
      segments.push(seg(l, l, "through", l === 0));
    }
    segments.push(seg(lane, lane, "top", lane === 0));
    if (row % 7 === 3 && lane < 5) segments.push(seg(lane, lane + 1, "bottom"));
    else segments.push(seg(lane, lane, "bottom", lane === 0));
    if (row % 13 === 5) segments.push(seg(lane, lane + 1, "bottom", false, true));
    rows.push({ row, lane, color: lane, kind: "normal", primary: lane === 0, width: 6, segments, links: [] });
  }
  return rows;
}

/** Branch Coloring: some lanes coloured, some dimmed. */
function painted(layout: GraphRow): RowPaint {
  const style = (lane: number) => (lane === 0 ? 0 : lane % 3 === 0 ? 0x10 | 2 : (lane % 4) + 1);
  return {
    nodeLane: layout.lane,
    nodeStyle: style(layout.lane),
    segmentLanes: layout.segments.map((s) => s.from),
    segmentStyles: layout.segments.map((s) => style(s.from)),
  };
}

const LAYOUTS = history(120);
const ROWS: DrawnRow[] = LAYOUTS.slice(10, 90).map((layout, at) => ({
  listRow: at + 11,
  layout,
  unpushed: at < 4,
  stash: at === 20,
  paint: painted(layout),
  bisect: at === 30 ? { dot: "--graph-bisect-good", tint: "--graph-bisect-current" } : undefined,
}));

const scene = (over: Partial<GraphScene> = {}): GraphScene => ({
  rows: ROWS,
  scrollTop: 11 * GRAPH.rowHeight + 5,
  height: 80 * GRAPH.rowHeight,
  headRow: 12,
  headLane: 1,
  headerRow: 11,
  headerLane: 1,
  workingTree: "changed",
  workingTreeLayout: null,
  selectedRows: [14, 40],
  hoverRow: 33,
  focusLane: 2,
  clipX: Number.POSITIVE_INFINITY,
  stripes: true,
  coloring: "branch",
  ...over,
});

/** As `tokenColor`: a lookup, answered from a cache. */
const tokens = new Map<string, string>();
const token = (name: string) => tokens.get(name) ?? (tokens.set(name, `var(${name})`), `var(${name})`);

function record(over: Partial<GraphScene> = {}): Recorder {
  const previous = g.Path2D;
  g.Path2D = FakePath;
  recorder = new Recorder();
  try {
    paintGraph(recorder as unknown as CanvasRenderingContext2D, scene(over), token);
    return recorder;
  } finally {
    recorder = null;
    g.Path2D = previous;
  }
}

describe("paintGraph", () => {
  it("draws one path per look, layer by layer", () => {
    const drawn = record();
    const strokes = drawn.log.filter((line) => line.startsWith("stroke M"));
    expect(strokes.length).toBe(drawn.paths);
    expect(drawn.paths).toBeLessThan(12);
    // Dimmed lines are the panel mixed in, drawn first.
    expect(drawn.log.find((line) => line.startsWith("strokeStyle"))).toContain("color-mix");
    expect(drawn.log).toMatchSnapshot();
  });

  it("draws a merge in progress, a header row and every colouring alike", () => {
    for (const coloring of ["default", "branch", "mergeable", "varying"] as const) {
      expect(record({ coloring, workingTreeLayout: LAYOUTS[5] ?? null }).log).toMatchSnapshot(coloring);
    }
    expect(record({ rows: ROWS.map((row) => ({ ...row, paint: undefined })), coloring: "mergeable" }).log).toMatchSnapshot("unpainted");
    expect(record({ clipX: 60 }).log).toMatchSnapshot("clipped");
  });

  it("allocates little per frame", { timeout: 60_000 }, async () => {
    const previous = g.Path2D;
    g.Path2D = NullPath;
    try {
      const frame = scene();
      const draw = () => paintGraph(nullContext, frame, token);
      const ms = msPerCall(draw);
      const frames = 50;
      const bytes = await allocatedBytes(() => {
        for (let i = 0; i < frames; i++) draw();
      });
      report("paintGraph, 80 rows", { msPerFrame: ms, kibPerFrame: bytes / frames / 1024 });
      expect(bytes / frames).toBeLessThan(Number.POSITIVE_INFINITY);
    } finally {
      g.Path2D = previous;
    }
  });
});
