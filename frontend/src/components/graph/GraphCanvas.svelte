<script lang="ts">
  import {
    GRAPH,
    arrowStub,
    canvasPixelSize,
    laneX,
    nodeCentre,
    nodeFill,
    nodeSquare,
    segmentCurve,
    textX,
  } from "$lib/graph-geometry";
  import { LAYERS, nodeStroke, segmentStroke, type RowPaint } from "$lib/graph-style";
  import { settings } from "$stores/settings.svelte";
  import type { GraphRow } from "$lib/ipc";

  /** Only the rows on screen: every segment belongs to its own row, so nothing outside
      the view is ever needed to draw it (doc/07-graph-rendering.md). */
  interface Props {
    rows: { listRow: number; layout: GraphRow; stash?: boolean; paint?: RowPaint }[];
    scrollTop: number;
    width: number;
    height: number;
    /** HEAD's list row and column: the dashed line from the Working Tree row ends there (T4.5). */
    headRow?: number | null;
    headLane?: number | null;
    /** A ring is filled with what is behind it: a stripe, a hovered or a selected row. */
    selectedRow?: number | null;
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
    selectedRow = null,
    hoverRow = null,
    focusLane = null,
    clipX = Number.POSITIVE_INFINITY,
    stripes = true,
    rowHeight = GRAPH.rowHeight,
  }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let dpr = $state(typeof window === "undefined" ? 1 : window.devicePixelRatio);
  let frame = 0;

  function draw() {
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;

    const pixels = canvasPixelSize(width, height, dpr);
    if (canvas.width !== pixels.width || canvas.height !== pixels.height) {
      canvas.width = pixels.width;
      canvas.height = pixels.height;
    }
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, width, height);
    context.imageSmoothingEnabled = true;
    context.lineCap = "round";

    const styles = getComputedStyle(document.documentElement);
    const token = (name: string) => styles.getPropertyValue(name).trim();
    const main = token("--graph-main");
    const line = token("--graph-line");
    const options = { colouredLanes: settings.current.coloredLanes, focusLane };
    const colours = new Map<string, string>();
    const colour = (name: string) => {
      if (!colours.has(name)) colours.set(name, token(name) || line);
      return colours.get(name) ?? line;
    };

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
          context.strokeStyle = colour(look.token);
          context.lineWidth = look.width;
          context.globalAlpha = look.alpha;
          context.beginPath();
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
    context.globalAlpha = 1;

    if (headLane !== null && headRow !== null && scrollTop < GRAPH.rowHeight * headRow) {
      const top = nodeCentre(headLane, 0, scrollTop);
      const foot = nodeCentre(headLane, headRow, scrollTop);
      context.save();
      context.setLineDash([3, 3]);
      context.strokeStyle = main;
      context.lineWidth = GRAPH.lineWidth;
      context.beginPath();
      context.moveTo(top.x, top.y);
      context.lineTo(foot.x, foot.y - GRAPH.ringRadius);
      context.stroke();
      context.restore();
    }

    // One hollow ring for every node, filled with what is behind it so no line shows through;
    // a stash is a square in the stash colour (#19).
    const panel = token("--surface-panel");
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
      for (const layer of nodeFill(row.listRow, selectedRow, hoverRow, stripes)) {
        if (!fills.has(layer)) fills.set(layer, token(layer));
        context.fillStyle = fills.get(layer) ?? panel;
        context.fill();
      }
      const ring = nodeStroke(row.layout, row.paint, options);
      context.lineWidth = GRAPH.ringStroke;
      context.strokeStyle = row.stash ? stash : colour(ring.token);
      context.globalAlpha = ring.alpha;
      context.stroke();
      context.globalAlpha = 1;
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
    void [rows, scrollTop, width, height, dpr, headRow, headLane, selectedRow, hoverRow, focusLane];
    void [clipX, stripes, rowHeight];
    void [settings.current.theme, settings.current.laneWidth, settings.current.coloredLanes];
    schedule();
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

<canvas bind:this={canvas} style:width="{width}px" style:height="{height}px"></canvas>

<style>
  canvas {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
  }
</style>
