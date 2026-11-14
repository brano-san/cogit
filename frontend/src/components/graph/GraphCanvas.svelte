<script lang="ts">
  import { GRAPH } from "$lib/graph-geometry";
  import { paintGraph, type DrawnRow } from "$lib/graph-draw";
  import { tokenColor } from "$lib/theme-colors";
  import { themeStore } from "$stores/theme.svelte";
  import { canvasBox, drawsNow, subpixelOffset, type CanvasBox } from "$lib/canvas-frame";
  import { settings } from "$stores/settings.svelte";
  import type { GraphRow } from "$lib/ipc";

  /** The rows on screen and the buffer around them: a turn between columns reaches half a
      row into the next one, so a row reads its neighbours' segments (doc/07-graph-rendering.md). */
  interface Props {
    /** `unpushed`: a dot with a gap around it; a commit some remote branch has is a hollow ring. */
    rows: readonly DrawnRow[];
    scrollTop: number;
    width: number;
    height: number;
    /** HEAD's list row and column: the dashed line from the Working Tree row ends there (T4.5). */
    headRow?: number | null;
    headLane?: number | null;
    /** Where the Working Tree row is: the top, or right above HEAD when HEAD is further down. */
    headerRow?: number;
    headerLane?: number | null;
    /** The Working Tree row's dot: green when clean, red when changed; none while unknown. */
    workingTree?: "clean" | "changed" | null;
    /** A merge in progress: the Working Tree row as the layout placed it, with its lines to
        HEAD and to the merged commit (07 §4). Drawn instead of the dashed line to HEAD. */
    workingTreeLayout?: GraphRow | null;
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
    workingTree = null,
    workingTreeLayout = null,
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

    paintGraph(
      context,
      {
        rows,
        scrollTop,
        height,
        headRow,
        headLane,
        headerRow,
        headerLane,
        workingTree,
        workingTreeLayout,
        selectedRows,
        hoverRow,
        focusLane,
        clipX,
        stripes,
        coloring: settings.current.graphColoring,
        grid: { ratio, x: off.x, y: off.y },
      },
      tokenColor,
    );
  }

  function schedule() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(draw);
  }

  $effect(() => {
    // Theme, lane width and colour change the picture without changing the data.
    void [rows, scrollTop, width, height, dpr, headRow, headLane, headerRow, headerLane, workingTree, workingTreeLayout, selectedRows, hoverRow, focusLane];
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
