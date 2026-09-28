<script lang="ts">
  import {
    GRAPH,
    arrowStub,
    laneX,
    nodeCentre,
    nodeFill,
    nodeSquare,
    segmentCurve,
    textX,
  } from "$lib/graph-geometry";
  import { LAYERS, nodeStroke, opaqueInk, segmentStroke, type RowPaint } from "$lib/graph-style";
  import { canvasBox, drawsNow, type CanvasBox } from "$lib/canvas-frame";
  import { settings } from "$stores/settings.svelte";
  import type { GraphRow } from "$lib/ipc";

  /** A bisect mark (F-566): the dot fills the ring, the tint is the row behind it. Tokens. */
  interface BisectPaint {
    dot: string | null;
    tint: string | null;
  }

  /** Only the rows on screen: every segment belongs to its own row, so nothing outside
      the view is ever needed to draw it (doc/07-graph-rendering.md). */
  interface Props {
    rows: { listRow: number; layout: GraphRow; stash?: boolean; paint?: RowPaint; bisect?: BisectPaint }[];
    scrollTop: number;
    width: number;
    height: number;
    /** HEAD's list row and column: the dashed line from the Working Tree row ends there (T4.5). */
    headRow?: number | null;
    headLane?: number | null;
    /** Where the Working Tree row is: the top, or right above HEAD when HEAD is further down. */
    headerRow?: number;
    headerLane?: number | null;
    /** A ring is filled with what is behind it: a stripe, a hovered or a selected row. */
    selectedRows?: readonly number[];
    hoverRow?: number | null;
    /** The lane drawn in front: the branch of the chosen commit. */
    focusLane?: number | null;
    /** Where the graph area is cut so the row's right columns fit (#12); a fade marks it. */
    clipX?: number;
    stripes?: boolean;
    /** Only to redraw when the density changes: geometry reads it from `GRAPH`. */
    rowHeight?: number;
  }

  let {
    rows,
    scrollTop,
    width,
    height,
    headRow = null,
    headLane = null,
    headerRow = 0,
    headerLane = null,
    selectedRows = [],
    hoverRow = null,
    focusLane = null,
    clipX = Number.POSITIVE_INFINITY,
    stripes = true,
    rowHeight = GRAPH.rowHeight,
  }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let dpr = $state(typeof window === "undefined" ? 1 : window.devicePixelRatio);
  let frame = 0;
  /** What is on screen: the bitmap and its CSS box change together, only here. */
  let drawnBox: CanvasBox | null = null;

  function draw() {
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;

    const box = canvasBox(width, height, dpr);
    if (canvas.width !== box.pixelWidth || canvas.height !== box.pixelHeight) {
      canvas.width = box.pixelWidth;
      canvas.height = box.pixelHeight;
    }
    canvas.style.width = `${box.cssWidth}px`;
    canvas.style.height = `${box.cssHeight}px`;
    drawnBox = box;
    const ratio = dpr > 0 ? dpr : 1;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, box.cssWidth, box.cssHeight);
    context.imageSmoothingEnabled = true;
    context.lineCap = "round";

    const styles = getComputedStyle(document.documentElement);
    const token = (name: string) => styles.getPropertyValue(name).trim();
    const main = token("--graph-main");
    const line = token("--graph-line");
    const coloring = settings.current.graphColoring;
    const options = { accentLit: coloring === "mergeable", branchOnly: coloring === "branch", focusLane };
    const panel = token("--surface-panel");
    const colours = new Map<string, string>();
    const colour = (name: string) => {
      if (!colours.has(name)) colours.set(name, token(name) || line);
      return colours.get(name) ?? line;
    };
    const ink = (name: string, alpha: number) => opaqueInk(colour(name), alpha, panel);

    const edge = Math.min(textX(GRAPH.maxColumns), clipX) - GRAPH.textGap;
    context.save();
    context.beginPath();
    context.rect(0, 0, edge, height);
    context.clip();

    // Grey first, branch colours over it, the main line on top: where lines cross, the one
    // the eye follows wins.
    for (let layer = 0; layer < LAYERS; layer++) {
      for (const row of rows) {
        for (const [index, segment] of row.layout.segments.entries()) {
          const look = segmentStroke(segment, index, row.paint, options);
          if (look.layer !== layer) continue;
          context.strokeStyle = ink(look.token, look.alpha);
          context.lineWidth = look.width;
          context.beginPath();
          if (headerRow > 0 && row.listRow === headRow && !segment.arrow && segment.span !== "bottom") {
            const x = laneX(segment.from);
            context.moveTo(x, headerRow * GRAPH.rowHeight - scrollTop);
            context.lineTo(x, row.listRow * GRAPH.rowHeight - scrollTop);
          }
          if (segment.arrow) {
            const stub = arrowStub(segment, row.listRow, scrollTop);
            context.moveTo(stub.x1, stub.y1);
            context.lineTo(stub.x2, stub.y2);
            context.moveTo(stub.left.x, stub.left.y);
            context.lineTo(stub.x2, stub.y2);
            context.lineTo(stub.right.x, stub.right.y);
          } else {
            const curve = segmentCurve(segment, row.listRow, scrollTop);
            context.moveTo(curve.x1, curve.y1);
            if (curve.x1 === curve.x2) context.lineTo(curve.x2, curve.y2);
            else context.bezierCurveTo(curve.cx1, curve.cy1, curve.cx2, curve.cy2, curve.x2, curve.y2);
          }
          context.stroke();
        }
      }
    }

    if (headLane !== null && headRow !== null && scrollTop < GRAPH.rowHeight * headRow) {
      const top = nodeCentre(headerLane ?? headLane, headerRow, scrollTop);
      const foot = nodeCentre(headLane, headRow, scrollTop);
      context.save();
      context.setLineDash([3, 3]);
      context.strokeStyle = main;
      context.lineWidth = GRAPH.lineWidth;
      context.beginPath();
      context.moveTo(top.x, top.y);
      if (top.x !== foot.x) {
        const bend = top.y + GRAPH.rowHeight / 2;
        const middle = (top.y + bend) / 2;
        context.bezierCurveTo(top.x, middle, foot.x, middle, foot.x, bend);
      }
      context.lineTo(foot.x, foot.y - GRAPH.ringRadius);
      context.stroke();
      context.restore();
    }

    // One hollow ring for every node, filled with what is behind it so no line shows through;
    // a stash is a square in the stash colour (#19).
    const stash = token("--status-stash");
    const fills = new Map<string, string>();
    for (const row of rows) {
      context.beginPath();
      if (row.stash) {
        const square = nodeSquare(row.layout.lane, row.listRow, scrollTop);
        context.rect(square.x, square.y, square.size, square.size);
      } else {
        const { x, y } = nodeCentre(row.layout.lane, row.listRow, scrollTop);
        context.arc(x, y, GRAPH.ringRadius, 0, Math.PI * 2);
      }
      const dot = row.bisect?.dot ?? null;
      const layers = nodeFill(row.listRow, selectedRows, hoverRow, stripes, row.bisect?.tint ?? null);
      for (const layer of dot ? [...layers, dot] : layers) {
        if (!fills.has(layer)) fills.set(layer, token(layer));
        context.fillStyle = fills.get(layer) ?? panel;
        context.fill();
      }
      const ring = nodeStroke(row.layout, row.paint, options);
      context.lineWidth = GRAPH.ringStroke;
      context.strokeStyle = row.stash ? stash : ink(ring.token, ring.alpha);
      context.stroke();
    }
    context.restore();

    // A row wider than the cap or the room left is cut off with a fade, not a hard edge.
    if (rows.some((row) => laneX(row.layout.width - 1) + GRAPH.laneWidth / 2 > edge)) {
      const fade = context.createLinearGradient(edge - GRAPH.laneWidth, 0, edge, 0);
      fade.addColorStop(0, "transparent");
      fade.addColorStop(1, panel);
      context.fillStyle = fade;
      context.fillRect(edge - GRAPH.laneWidth, 0, GRAPH.laneWidth, height);
    }
  }

  function schedule() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(draw);
  }

  $effect(() => {
    // Theme, lane width and colour change the picture without changing the data.
    void [rows, scrollTop, width, height, dpr, headRow, headLane, headerRow, headerLane, selectedRows, hoverRow, focusLane];
    void [clipX, stripes, rowHeight];
    void [settings.current.theme, settings.current.laneWidth, settings.current.graphColoring];
    if (drawsNow(drawnBox, canvasBox(width, height, dpr))) {
      cancelAnimationFrame(frame);
      draw();
    } else {
      schedule();
    }
    return () => cancelAnimationFrame(frame);
  });

  $effect(() => {
    const query = window.matchMedia(`(resolution: ${dpr}dppx)`);
    const onChange = () => {
      dpr = window.devicePixelRatio;
    };
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  });
</script>

<canvas bind:this={canvas}></canvas>

<style>
  canvas {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
  }
</style>
