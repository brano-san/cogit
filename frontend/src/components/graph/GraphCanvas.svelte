<script lang="ts">
  import { GRAPH, canvasPixelSize, edgeBand, indexByRow, nodeCentre } from "$lib/graph-geometry";
  import { settings } from "$stores/settings.svelte";
  import type { GraphEdge } from "$lib/ipc";

  interface Props {
    edges: GraphEdge[];
    nodes: { row: number; lane: number; color: number; merge: boolean; root: boolean }[];
    scrollTop: number;
    width: number;
    height: number;
    firstRow: number;
    lastRow: number;
    rowOffset: number;
    /** The lane HEAD sits in, for the dashed edge from the working-tree row (T4.5). */
    headLane?: number | null;
  }

  let {
    edges,
    nodes,
    scrollTop,
    width,
    height,
    firstRow,
    lastRow,
    rowOffset,
    headLane = null,
  }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let dpr = $state(typeof window === "undefined" ? 1 : window.devicePixelRatio);
  let frame = 0;

  const edgesByRow = $derived(indexByRow(edges));

  function laneColor(index: number): string {
    const styles = getComputedStyle(document.documentElement);
    return styles.getPropertyValue(`--c-lane-${(index % 8) + 1}`).trim() || "#38bdf8";
  }

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
    context.lineWidth = GRAPH.lineWidth;

    const band = edgeBand(edgesByRow, firstRow, lastRow);
    const order = { crossing: 0, direct: 1, merge: 2 } as const;
    band.sort((a, b) => order[a.kind] - order[b.kind]);

    for (const edge of band) {
      const { x: x1, y: y1 } = nodeCentre(edge.fromLane, edge.fromRow + rowOffset, scrollTop);
      const { x: x2, y: y2 } = nodeCentre(edge.toLane, edge.toRow + rowOffset, scrollTop);

      context.strokeStyle = laneColor(edge.color);
      context.beginPath();
      context.moveTo(x1, y1);
      if (x1 === x2) {
        context.lineTo(x2, y2);
      } else {
        const bend = (y2 - y1) / 2;
        context.bezierCurveTo(x1, y1 + bend, x2, y2 - bend, x2, y2);
      }
      context.stroke();
    }

    if (headLane !== null && firstRow === 0) {
      const top = nodeCentre(headLane, 0, scrollTop);
      const foot = nodeCentre(headLane, rowOffset, scrollTop);
      context.save();
      context.setLineDash([3, 3]);
      context.strokeStyle = laneColor(0);
      context.beginPath();
      context.moveTo(top.x, top.y);
      context.lineTo(foot.x, foot.y);
      context.stroke();
      context.restore();
    }

    for (const node of nodes) {
      const { x, y } = nodeCentre(node.lane, node.row + rowOffset, scrollTop);
      context.fillStyle = laneColor(node.color);
      context.beginPath();
      if (node.root) {
        const size = GRAPH.nodeRadius * 1.6;
        context.rect(x - size / 2, y - size / 2, size, size);
      } else {
        context.arc(x, y, node.merge ? GRAPH.mergeRadius : GRAPH.nodeRadius, 0, Math.PI * 2);
      }
      context.fill();
      if (node.merge) {
        context.strokeStyle = getComputedStyle(document.documentElement)
          .getPropertyValue("--surface-panel")
          .trim();
        context.stroke();
      }
    }
  }

  function schedule() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(draw);
  }

  $effect(() => {
    // The theme and the lane width change what is painted without changing the data.
    void [edges, nodes, scrollTop, width, height, dpr, rowOffset, headLane];
    void [settings.current.theme, settings.current.laneWidth];
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
