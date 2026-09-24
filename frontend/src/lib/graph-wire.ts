import type { CommitRow, GraphRow, Segment } from "$lib/ipc";

/** A commit as the list shows it; parents stay in Rust, the list never draws from them. */
export type GraphCommit = Omit<CommitRow, "parents">;

export interface GraphEntry {
  commit: GraphCommit;
  layout: GraphRow;
}

/** A window of the graph as it came over: columns over one buffer, decoded row by row. */
export interface GraphBlock {
  start: number;
  total: number;
  complete: boolean;
  length: number;
  oid(row: number): string;
  /** The row of `oid` in this block, or -1. */
  find(oid: string): number;
  entry(row: number): GraphEntry;
}

const VERSION = 2;
const KINDS: readonly GraphRow["kind"][] = ["normal", "merge", "root", "workingTree"];
const SPANS: readonly Segment["span"][] = ["top", "bottom", "through"];
const utf8 = new TextDecoder();

type View<T> = new (buffer: ArrayBuffer, offset: number, length: number) => T;

/** The window as it crosses `postMessage`: base64 of the columns (R-194). */
export function decodeBase64Window(text: string): GraphBlock | null {
  const native = (Uint8Array as unknown as { fromBase64?: (s: string) => Uint8Array }).fromBase64;
  if (native) return decodeWindow(native(text).buffer as ArrayBuffer);
  const raw = atob(text);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return decodeWindow(bytes.buffer);
}

/** The layout is in `crates/app_state/src/graph_wire.rs`. An empty buffer is a window
    of a graph that was replaced before it was asked for. */
export function decodeWindow(buffer: ArrayBuffer): GraphBlock | null {
  if (buffer.byteLength === 0) return null;
  const [version, start, rows, total, complete, segments, textBytes, oidBytes] = new Uint32Array(buffer, 0, 8);
  if (version !== VERSION) throw new Error(`graph window format ${version}, expected ${VERSION}`);

  let at = 32;
  function take<T>(Kind: View<T> & { BYTES_PER_ELEMENT: number }, length: number): T {
    const view = new Kind(buffer, at, length);
    at += length * Kind.BYTES_PER_ELEMENT;
    return view;
  }
  const time = take(Float64Array, rows!);
  const firstSegment = take(Uint32Array, rows! + 1);
  const firstLink = take(Uint32Array, rows! + 1);
  const links = firstLink[rows!]!;
  const textAt = take(Uint32Array, rows! * 3 + 1);
  const zone = take(Int32Array, rows!);
  const lane = take(Uint16Array, rows!);
  const width = take(Uint16Array, rows!);
  const from = take(Uint16Array, segments!);
  const to = take(Uint16Array, segments!);
  const linkSegment = take(Uint16Array, links);
  const colour = take(Uint8Array, rows!);
  const flags = take(Uint8Array, rows!);
  const segmentColour = take(Uint8Array, segments!);
  const segmentFlags = take(Uint8Array, segments!);
  const oids = take(Uint8Array, rows! * oidBytes!);
  const linkOids = take(Uint8Array, links * oidBytes!);
  const text = take(Uint8Array, textBytes!);

  const field = (row: number, which: number) =>
    utf8.decode(text.subarray(textAt[row * 3 + which], textAt[row * 3 + which + 1]));
  const oid = (row: number) => utf8.decode(oids.subarray(row * oidBytes!, (row + 1) * oidBytes!));
  const decoded: GraphEntry[] = [];
  // Hex ids are ASCII: one decode of the whole column, sliced at fixed width.
  let column: string | null = null;

  return {
    start: start!,
    total: total!,
    complete: complete === 1,
    length: rows!,
    oid,
    find(wanted) {
      if (wanted.length !== oidBytes) return -1;
      column ??= utf8.decode(oids);
      for (let at = column.indexOf(wanted); at >= 0; at = column.indexOf(wanted, at + 1)) {
        if (at % oidBytes === 0) return at / oidBytes;
      }
      return -1;
    },
    entry(row) {
      const known = decoded[row];
      if (known) return known;
      const drawn: Segment[] = [];
      for (let s = firstSegment[row]!; s < firstSegment[row + 1]!; s++) {
        const bits = segmentFlags[s]!;
        drawn.push({
          from: from[s]!,
          to: to[s]!,
          span: SPANS[bits & 3]!,
          primary: (bits & 4) !== 0,
          color: segmentColour[s]!,
          arrow: (bits & 8) !== 0,
        });
      }
      const far: GraphRow["links"] = [];
      for (let l = firstLink[row]!; l < firstLink[row + 1]!; l++) {
        far.push({ segment: linkSegment[l]!, oid: utf8.decode(linkOids.subarray(l * oidBytes!, (l + 1) * oidBytes!)) });
      }
      const entry: GraphEntry = {
        commit: {
          oid: oid(row),
          summary: field(row, 0),
          authorName: field(row, 1),
          authorEmail: field(row, 2),
          timestamp: time[row]!,
          tzOffsetMinutes: zone[row]!,
        },
        layout: {
          row: start! + row,
          lane: lane[row]!,
          color: colour[row]!,
          kind: KINDS[flags[row]! & 3]!,
          primary: (flags[row]! & 4) !== 0,
          width: width[row]!,
          segments: drawn,
          links: far,
        },
      };
      decoded[row] = entry;
      return entry;
    },
  };
}
