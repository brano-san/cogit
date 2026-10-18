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
    transition,
  } from "$lib/graph-geometry";
  import { tokenColor } from "$lib/theme-colors";
  import { themeStore } from "$stores/theme.svelte";
  import { LAYERS, nodeStroke, opaqueInk, segmentStroke, type RowPaint } from "$lib/graph-style";
  import { canvasBox, drawsNow, subpixelOffset, type CanvasBox } from "$lib/canvas-frame";
  import { settings } from "$stores/settings.svelte";
  import type { GraphRow } from "$lib/ipc";

  /** A bisect mark (F-566): the dot fills the ring, the tint is the row behind it. Tokens. */
  interface BisectPaint {
    dot: string | null;
    tint: string | null;
  }

  /** The rows on screen and the buffer around them: a turn between columns reaches half a
      row into the next one, so a row reads its neighbours' segments (doc/07-graph-rendering.md). */
  interface Props {
    /** `unpushed`: a dot with a gap around it; a commit some remote branch has is a hollow ring. */
    rows: {
      listRow: number;
      layout: GraphRow;
      stash?: boolean;
      unpushed?: boolean;
      paint?: RowPaint;
      bisect?: BisectPaint;
    }[];
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
    // Off the device-pixel grid the compositor resamples the bitmap and blurs every line: the
    // canvas moves back onto the grid and the drawing forward by as much, level with the rows.
    const host = canvas.parentElement?.getBoundingClientRect();
    const off = host ? subpixelOffset(host.left, host.top, ratio) : { x: 0, y: 0 };
    canvas.style.transform = off.x || off.y ? `translate(${-off.x / ratio}px, ${-off.y / ratio}px)` : "";
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.clearRect(0, 0, canvas.width, canvas.height);
    context.setTransform(ratio, 0, 0, ratio, off.x, off.y);
    context.imageSmoothingEnabled = true;
    context.lineCap = "round";
    context.lineJoin = "round";

    const token = tokenColor;
    const main = token("--graph-main");
    const line = token("--graph-line");
    const coloring = settings.current.graphColoring;
    const options = { accentLit: coloring === "mergeable", branchOnly: coloring === "branch", focusLane };
    const panel = token("--bg-editor");
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

    const segmentsAt = new Map(rows.map((row) => [row.listRow, row.layout.segments]));
    // Grey first, branch colours over it, the main line on top: where lines cross, the one
    // the eye follows wins.
    for (let layer = 0; layer < LAYERS; layer++) {
      // One path per look: a line's segments meet at every row edge, and stroked one by one
      // their round caps were painted twice there, a tick across the line on every row.
      const strokes = new Map<string, { style: string; width: number; path: Path2D }>();
      for (const row of rows) {
        for (const [index, segment] of row.layout.segments.entries()) {
          const look = segmentStroke(segment, index, row.paint, options);
          if (look.layer !== layer) continue;
          const style = ink(look.token, look.alpha);
          const key = `${style} ${look.width}`;
          let stroke = strokes.get(key);
          if (!stroke) {
            stroke = { style, width: look.width, path: new Path2D() };
            strokes.set(key, stroke);
          }
          const { path } = stroke;
          const curve = segment.arrow
            ? null
            : segmentCurve(segment, row.listRow, scrollTop, {
                above: segmentsAt.get(row.listRow - 1),
                below: segmentsAt.get(row.listRow + 1),
              });
          // Across the Working Tree row to where the line in HEAD's row starts.
          if (curve && headerRow > 0 && row.listRow === headRow && segment.span !== "bottom") {
            const x = laneX(segment.from);
            path.moveTo(x, headerRow * GRAPH.rowHeight - scrollTop);
            path.lineTo(x, Math.min(curve.y1, curve.y2));
          }
          if (segment.arrow) {
            const stub = arrowStub(segment, row.listRow, scrollTop);
            path.moveTo(stub.x1, stub.y1);
            path.lineTo(stub.x2, stub.y2);
            path.moveTo(stub.left.x, stub.left.y);
            path.lineTo(stub.x2, stub.y2);
            path.lineTo(stub.right.x, stub.right.y);
          } else if (curve) {
            path.moveTo(curve.x1, curve.y1);
            if (curve.x1 === curve.x2) path.lineTo(curve.x2, curve.y2);
            else path.bezierCurveTo(curve.cx1, curve.cy1, curve.cx2, curve.cy2, curve.x2, curve.y2);
          }
        }
      }
      for (const { style, width, path } of strokes.values()) {
        context.strokeStyle = style;
        context.lineWidth = width;
        context.stroke(path);
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
        const turn = transition(top, foot.x, "down");
        context.bezierCurveTo(turn.cx1, turn.cy1, turn.cx2, turn.cy2, turn.x2, turn.y2);
      }
      context.lineTo(foot.x, foot.y);
      context.stroke();
      context.restore();
    }

    // A commit no remote has is a dot in its own colour, the lines stopping just short of it;
    // a pushed one is a hollow ring, filled with what is behind it so no line shows through.
    // A stash is a square in the stash colour (#19).
    const stash = token("--status-stash");
    const fills = new Map<string, string>();
    const fillBehind = (layers: string[]) => {
      for (const layer of layers) {
        if (!fills.has(layer)) fills.set(layer, token(layer));
        context.fillStyle = fills.get(layer) ?? panel;
        context.fill();
      }
    };
    for (const row of rows) {
      const local = !row.stash && row.unpushed === true;
      const behind = nodeFill(row.listRow, selectedRows, hoverRow, stripes, row.bisect?.tint ?? null);
      if (local) {
        const { x, y } = nodeCentre(row.layout.lane, row.listRow, scrollTop);
        context.beginPath();
        context.arc(x, y, GRAPH.ringRadius + GRAPH.ringStroke / 2 + GRAPH.localGap, 0, Math.PI * 2);
        fillBehind(behind);
      }
      context.beginPath();
      if (row.stash) {
        const square = nodeSquare(row.layout.lane, row.listRow, scrollTop);
        context.rect(square.x, square.y, square.size, square.size);
      } else {
        const { x, y } = nodeCentre(row.layout.lane, row.listRow, scrollTop);
        context.arc(x, y, GRAPH.ringRadius, 0, Math.PI * 2);
      }
      const ring = nodeStroke(row.layout, row.paint, options);
      const ringInk = row.stash ? stash : ink(ring.token, ring.alpha);
      const dot = row.bisect?.dot ?? null;
      fillBehind(behind);
      if (local) {
        context.fillStyle = ringInk;
        context.fill();
      }
      if (dot) {
        if (!fills.has(dot)) fills.set(dot, token(dot));
        context.fillStyle = fills.get(dot) ?? panel;
        context.fill();
      }
      context.lineWidth = GRAPH.ringStroke;
      context.strokeStyle = ringInk;
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
    void [themeStore.version, settings.current.theme, settings.current.laneWidth, settings.current.graphColoring];
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
