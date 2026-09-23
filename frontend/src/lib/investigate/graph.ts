export interface GraphRow {
  lane: number;
  through: number[];
  /** A newer row leads down into this node. */
  fromAbove: boolean;
  /** Side-branch lanes that end in this node. */
  joins: number[];
  toParents: number[];
  width: number;
}

export type Point = readonly [number, number];

export function segmentsOf(
  row: GraphRow,
  laneWidth: number,
  height: number,
): { node: Point; lines: [Point, Point][] } {
  const x = (lane: number) => laneWidth / 2 + lane * laneWidth;
  const node: Point = [x(row.lane), height / 2];
  const lines: [Point, Point][] = [];
  for (const lane of row.through) lines.push([[x(lane), 0], [x(lane), height]]);
  if (row.fromAbove) lines.push([[x(row.lane), 0], node]);
  for (const lane of row.joins) lines.push([[x(lane), 0], node]);
  for (const lane of row.toParents) lines.push([node, [x(lane), height]]);
  return { node, lines };
}

interface Commitish {
  oid: string;
  parents: readonly string[];
}

/** Lanes for a file's history, newest first. `git log --follow` lists real parents, not
    rewritten ones, so a row with no listed parent is joined to the next row. */
export function layoutGraph(rows: readonly Commitish[]): GraphRow[] {
  const position = new Map(rows.map((row, index) => [row.oid, index]));
  const active: (string | null)[] = [];
  const free = () => {
    const slot = active.indexOf(null);
    return slot < 0 ? active.length : slot;
  };

  return rows.map((row, index) => {
    let lane = active.indexOf(row.oid);
    const fromAbove = lane >= 0;
    if (lane < 0) lane = free();

    const joins: number[] = [];
    active.forEach((expected, slot) => {
      if (slot !== lane && expected === row.oid) joins.push(slot);
    });
    for (const slot of joins) active[slot] = null;
    const through = active.flatMap((expected, slot) =>
      expected !== null && slot !== lane ? [slot] : [],
    );

    let parents = row.parents.filter((parent) => (position.get(parent) ?? -1) > index);
    const next = rows[index + 1];
    if (parents.length === 0 && next) parents = [next.oid];

    active[lane] = parents[0] ?? null;
    const toParents = parents.length > 0 ? [lane] : [];
    for (const parent of parents.slice(1)) {
      let slot = active.indexOf(parent);
      if (slot < 0) {
        slot = free();
        active[slot] = parent;
      }
      toParents.push(slot);
    }

    const width = Math.max(lane, ...through, ...joins, ...toParents) + 1;
    while (active.length > 0 && active[active.length - 1] === null) active.pop();
    return { lane, through, fromAbove, joins, toParents, width };
  });
}
