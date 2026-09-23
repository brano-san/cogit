<script lang="ts">
  import {
    GRAPH,
    arrowStub,
    canvasPixelSize,
    laneX,
    nodeCentre,
    nodeSquare,
    segmentCurve,
    textX,
  } from "$lib/graph-geometry";
  import { settings } from "$stores/settings.svelte";
  import type { GraphRow } from "$lib/ipc";

  /** Only the rows on screen: every segment belongs to its own row, so nothing outside
      the view is ever needed to draw it (doc/07-graph-rendering.md). */
  interface Props {
    rows: { listRow: number; layout: GraphRow; stash?: boolean }[];
    scrollTop: number;
    width: number;
    height: number;
    /** The list row of the first commit, below the Working Tree row. */
    firstCommitRow: number;
    /** The column HEAD sits in, for the dashed line from the Working Tree row (T4.5). */
    headLane?: number | null;
    /** A ring is filled with what is behind it, and a selected row is a different colour. */
    selectedRow?: number | null;
  }

  let {
    rows,
    scrollTop,
    width,
    height,
    firstCommitRow,
    headLane = null,
    selectedRow = null,
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
    const colored = settings.current.coloredLanes;
    const stroke = (primary: boolean, color: number) =>
      colored ? token(`--c-lane-${(color % 8) + 1}`) || line : primary ? main : line;

    const edge = textX(GRAPH.maxColumns) - GRAPH.textGap;
    context.save();
    context.beginPath();
    context.rect(0, 0, edge, height);
    context.clip();

    // Grey first, the main line over it: where they cross, the one the eye follows wins.
    for (const primary of [false, true]) {
      for (const row of rows) {
        for (const segment of row.layout.segments) {
          if (segment.primary !== primary) continue;
          context.strokeStyle = stroke(segment.primary, segment.color);
          context.lineWidth = primary ? GRAPH.mainLineWidth : GRAPH.lineWidth;
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

    if (headLane !== null && scrollTop < GRAPH.rowHeight * firstCommitRow) {
      const top = nodeCentre(headLane, 0, scrollTop);
      const foot = nodeCentre(headLane, firstCommitRow, scrollTop);
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
    const selection = token("--state-selected");
    const stash = token("--status-stash");
    for (const row of rows) {
      context.beginPath();
      if (row.stash) {
        const square = nodeSquare(row.layout.lane, row.listRow, scrollTop);
        context.rect(square.x, square.y, square.size, square.size);
      } else {
        const { x, y } = nodeCentre(row.layout.lane, row.listRow, scrollTop);
        context.arc(x, y, GRAPH.ringRadius, 0, Math.PI * 2);
      }
      context.fillStyle = panel;
      context.fill();
      if (row.listRow === selectedRow) {
        context.fillStyle = selection;
        context.fill();
      }
      context.lineWidth = GRAPH.ringStroke;
      context.strokeStyle = row.stash ? stash : stroke(row.layout.primary, row.layout.color);
      context.stroke();
    }
    context.restore();

    // A row wider than the cap is cut off with a fade, not a hard edge.
    if (rows.some((row) => row.layout.width > GRAPH.maxColumns)) {
      const fade = context.createLinearGradient(edge - GRAPH.laneWidth, 0, edge, 0);
      fade.addColorStop(0, "transparent");
      fade.addColorStop(1, panel);
      context.fillStyle = fade;
      context.fillRect(laneX(GRAPH.maxColumns - 1), 0, GRAPH.laneWidth, height);
    }
  }

  function schedule() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(draw);
  }

  $effect(() => {
    // Theme, lane width and colour change the picture without changing the data.
    void [rows, scrollTop, width, height, dpr, firstCommitRow, headLane, selectedRow];
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
