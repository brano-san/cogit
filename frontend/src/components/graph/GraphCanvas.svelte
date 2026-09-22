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
        const middle = (y1 + y2) / 2;
        const radius = Math.min(GRAPH.elbowRadius, Math.abs(x2 - x1) / 2, Math.abs(y2 - y1) / 2);
        context.arcTo(x1, middle, x2, middle, radius);
        context.arcTo(x2, middle, x2, y2, radius);
        context.lineTo(x2, y2);
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

    const behind = getComputedStyle(document.documentElement)
      .getPropertyValue("--surface-panel")
      .trim();

    for (const node of nodes) {
      const { x, y } = nodeCentre(node.lane, node.row + rowOffset, scrollTop);
      context.beginPath();
      if (node.root) {
        const size = GRAPH.nodeRadius * 1.6;
        context.rect(x - size / 2, y - size / 2, size, size);
        context.fillStyle = laneColor(node.color);
        context.fill();
        continue;
      }

      if (node.merge) {
        // A ring, not a disc: the lane passes through the middle of it.
        context.arc(x, y, GRAPH.mergeRadius, 0, Math.PI * 2);
        context.fillStyle = behind;
        context.fill();
        context.strokeStyle = laneColor(node.color);
        context.lineWidth = GRAPH.ringWidth;
        context.stroke();
        context.lineWidth = GRAPH.lineWidth;
        continue;
      }

      context.arc(x, y, GRAPH.nodeRadius, 0, Math.PI * 2);
      context.fillStyle = laneColor(node.color);
      context.fill();
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
