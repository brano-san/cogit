<script lang="ts">
  import { GRAPH, canvasPixelSize, laneX, rowY } from "$lib/graph-geometry";
  import type { GraphEdge } from "$lib/ipc";

  interface Props {
    edges: GraphEdge[];
    nodes: { row: number; lane: number; color: number; merge: boolean; root: boolean }[];
    scrollTop: number;
    width: number;
    height: number;
    firstRow: number;
    lastRow: number;
    /** Rows the list draws above the first commit; edges and nodes shift by it. */
    rowOffset: number;
  }

  let { edges, nodes, scrollTop, width, height, firstRow, lastRow, rowOffset }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let dpr = $state(typeof window === "undefined" ? 1 : window.devicePixelRatio);
  let frame = 0;

  /**
   * Edges bucketed by their upper row.
   *
   * Scanning the whole array each frame would be O(commits) per frame; at 50 000
   * commits that alone misses the frame budget.
   */
  const edgesByRow = $derived.by(() => {
    const index = new Map<number, GraphEdge[]>();
    for (const edge of edges) {
      const bucket = index.get(edge.fromRow);
      if (bucket) bucket.push(edge);
      else index.set(edge.fromRow, [edge]);
    }
    return index;
  });

  /** Lane colours live in CSS so the palette stays in one place. */
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

    // Edges first, then nodes: otherwise lines are drawn across the dots.
    // Within edges, crossings go underneath so active branches stay legible.
    const band: GraphEdge[] = [];
    for (let row = firstRow - 1; row <= lastRow; row++) {
      const bucket = edgesByRow.get(row);
      if (bucket) band.push(...bucket);
    }
    const order = { crossing: 0, direct: 1, merge: 2 } as const;
    band.sort((a, b) => order[a.kind] - order[b.kind]);

    for (const edge of band) {
      const x1 = laneX(edge.fromLane);
      const y1 = rowY(edge.fromRow + rowOffset, scrollTop);
      const x2 = laneX(edge.toLane);
      const y2 = rowY(edge.toRow + rowOffset, scrollTop);

      context.strokeStyle = laneColor(edge.color);
      context.beginPath();
      // Half-pixel offset keeps a 1px line on one pixel instead of smeared across two.
      context.moveTo(x1 + 0.5, y1 + 0.5);
      if (x1 === x2) {
        context.lineTo(x2 + 0.5, y2 + 0.5);
      } else {
        const bend = (y2 - y1) / 2;
        context.bezierCurveTo(
          x1 + 0.5,
          y1 + bend + 0.5,
          x2 + 0.5,
          y2 - bend + 0.5,
          x2 + 0.5,
          y2 + 0.5,
        );
      }
      context.stroke();
    }

    for (const node of nodes) {
      const x = laneX(node.lane);
      const y = rowY(node.row + rowOffset, scrollTop);
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
    // Drawing straight from a scroll handler desynchronises the canvas from the DOM;
    // one draw per frame keeps them together (doc/07-graph-rendering.md).
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(draw);
  }

  $effect(() => {
    // Touch every input so the effect reruns when any of them changes.
    void [edges, nodes, scrollTop, width, height, dpr, rowOffset];
    schedule();
    return () => cancelAnimationFrame(frame);
  });

  $effect(() => {
    // Moving the window to a monitor with a different scaling changes the ratio.
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
