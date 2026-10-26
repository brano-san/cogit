import { shortOid } from "$lib/format";
import { clickedCommit } from "$lib/graph-geometry";
export type DragKind = "commit" | "branch" | "remote" | "tag" | "step";

/** Past this many pixels a press is a drag; short of it, it stays a click. */
export const DRAG_THRESHOLD = 4;

/** A row dragged on pointer events (`lib/pointer-drag.ts`): WebView2 gives HTML5
    drag-and-drop to Tauri's native file drop, and `drop` never reaches the page (R-450). */
export interface PointerDrag {
  readonly source: string;
  readonly x: number;
  readonly y: number;
  readonly moving: boolean;
}

export function pressDrag(source: string, x: number, y: number): PointerDrag {
  return { source, x, y, moving: false };
}

export function moveDrag(drag: PointerDrag, x: number, y: number, threshold = DRAG_THRESHOLD): PointerDrag {
  if (drag.moving || Math.hypot(x - drag.x, y - drag.y) <= threshold) return drag;
  return { ...drag, moving: true };
}

export function ghostAt(
  drag: PointerDrag,
  row: { left: number; top: number },
  x: number,
  y: number,
): { left: number; top: number } {
  return { left: row.left + x - drag.x, top: row.top + y - drag.y };
}

/** Where a release drops: nowhere for a click, off every row, or back on its own row. */
export function dropOn(drag: PointerDrag | null, target: string | null): string | null {
  if (!drag?.moving || target === null || target === drag.source) return null;
  return target;
}

/** The commit under `y` (from the top of the graph's viewport). The Working Tree row, the
    rows of a rebase in flight and a commit not loaded yet are nothing to drop on. */
export function graphDropTarget(
  y: number,
  scrollTop: number,
  rowHeight: number,
  listRows: number,
  headerRows: number,
  oidAt: (commitRow: number) => string | undefined,
  at = 0,
): string | null {
  if (y < 0 || rowHeight <= 0) return null;
  const row = Math.floor((y + scrollTop) / rowHeight);
  if (row >= listRows) return null;
  return clickedCommit(row, headerRows, oidAt, at) ?? null;
}

export interface DragPayload {
  kind: DragKind;
  id: string;
}

export type DropActionId =
  | "merge"
  | "rebase"
  | "fastForward"
  | "squash"
  | "reorder"
  | "setUpstream"
  | "moveBranch"
  | "moveTag"
  | "cherryPick";

export interface DropAction {
  id: DropActionId;
  title: string;
  destructive: boolean;
  disabled?: string;
  subject: string;
  object: string;
}

export interface DropContext {
  head: string | null;
  oidOf: (item: DragPayload) => string | undefined;
  upstreamOf: (branch: string) => string | null;
  canFastForward: (branch: string, to: DragPayload) => boolean;
}

const KINDS: readonly DragKind[] = ["commit", "branch", "remote", "tag", "step"];

export function serialiseDrag(payload: DragPayload): string {
  return `${payload.kind}:${payload.id}`;
}

export function parseDrag(text: string): DragPayload | null {
  const colon = text.indexOf(":");
  if (colon < 0) return null;
  const kind = text.slice(0, colon) as DragKind;
  const id = text.slice(colon + 1);
  return KINDS.includes(kind) && id !== "" ? { kind, id } : null;
}

export function labelDrag(label: { kind: string; ref?: string }): string | null {
  if (label.ref === undefined) return null;
  const kind: DragKind | null =
    label.kind === "head" || label.kind === "local" ? "branch" : label.kind === "remote" || label.kind === "tag" ? label.kind : null;
  return kind ? serialiseDrag({ kind, id: label.ref }) : null;
}

export function nodeDrag(node: { branch?: { kind: string; name: string }; tag?: { name: string } }): string | null {
  if (node.branch) return serialiseDrag({ kind: node.branch.kind === "local" ? "branch" : "remote", id: node.branch.name });
  return node.tag ? serialiseDrag({ kind: "tag", id: node.tag.name }) : null;
}

export function nameOf(item: DragPayload): string {
  return item.kind === "commit" ? shortOid(item.id) : item.id;
}

function checkedOut(branch: string, head: string | null, why: string): string | undefined {
  return branch === head ? undefined : `Check out ${branch} first: ${why}`;
}

function merge(source: DragPayload, into: string, head: string | null): DropAction {
  return {
    id: "merge",
    title: `Merge ${nameOf(source)} into ${into}`,
    destructive: false,
    disabled: checkedOut(into, head, "a merge goes into the checked-out branch."),
    subject: source.id,
    object: into,
  };
}

function rebase(branch: string, onto: DragPayload, head: string | null): DropAction {
  return {
    id: "rebase",
    title: `Rebase ${branch} onto ${nameOf(onto)}`,
    destructive: true,
    disabled: checkedOut(branch, head, "a rebase moves the checked-out branch."),
    subject: branch,
    object: onto.id,
  };
}

function moveBranch(branch: string, to: DragPayload, context: DropContext): DropAction {
  const here = context.oidOf({ kind: "branch", id: branch });
  const there = context.oidOf(to);
  return {
    id: "moveBranch",
    title: `Move ${branch} to ${nameOf(to)}`,
    destructive: true,
    disabled: here !== undefined && here === there ? `${branch} is already there.` : undefined,
    subject: branch,
    object: there ?? to.id,
  };
}

function moveTag(tag: string, to: DragPayload, context: DropContext): DropAction {
  const here = context.oidOf({ kind: "tag", id: tag });
  const there = context.oidOf(to);
  return {
    id: "moveTag",
    title: `Move tag ${tag} to ${nameOf(to)}`,
    destructive: true,
    disabled: here !== undefined && here === there ? `${tag} is already there.` : undefined,
    subject: tag,
    object: there ?? to.id,
  };
}

function setUpstream(branch: string, upstream: string, context: DropContext): DropAction {
  return {
    id: "setUpstream",
    title: `Set the upstream of ${branch} to ${upstream}`,
    destructive: false,
    disabled: context.upstreamOf(branch) === upstream ? `${branch} already tracks ${upstream}.` : undefined,
    subject: branch,
    object: upstream,
  };
}

function fastForward(branch: string, to: DragPayload, context: DropContext): DropAction[] {
  if (!context.canFastForward(branch, to)) return [];
  return [{ id: "fastForward", title: `Fast-forward ${branch} to ${nameOf(to)}`, destructive: false, subject: to.id, object: branch }];
}

export function dropActions(source: DragPayload, target: DragPayload, context: DropContext): DropAction[] {
  if (source.kind === target.kind && source.id === target.id) return [];
  const { head } = context;
  const key = `${source.kind}>${target.kind}`;
  switch (key) {
    case "branch>branch":
      return [merge(source, target.id, head), rebase(source.id, target, head), ...fastForward(target.id, source, context)];
    case "branch>remote":
      return [setUpstream(source.id, target.id, context), rebase(source.id, target, head)];
    case "remote>branch":
      return [merge(source, target.id, head), setUpstream(target.id, source.id, context), ...fastForward(target.id, source, context)];
    case "branch>commit":
    case "branch>tag":
      return [rebase(source.id, target, head), moveBranch(source.id, target, context)];
    case "tag>branch":
      return [merge(source, target.id, head), moveTag(source.id, target, context)];
    case "tag>remote":
    case "tag>tag":
    case "tag>commit":
      return [moveTag(source.id, target, context)];
    case "commit>branch":
      return [
        {
          id: "cherryPick",
          title: `Cherry-pick ${shortOid(source.id)} onto ${target.id}`,
          destructive: false,
          disabled: checkedOut(target.id, head, "a cherry-pick adds to the checked-out branch."),
          subject: source.id,
          object: target.id,
        },
        moveBranch(target.id, source, context),
      ];
    case "commit>tag":
      return [moveTag(target.id, source, context)];
    case "commit>commit":
      return [
        { id: "squash", title: `Squash ${shortOid(source.id)} into ${shortOid(target.id)}`, destructive: true, subject: source.id, object: target.id },
        { id: "reorder", title: `Move ${shortOid(source.id)} next to ${shortOid(target.id)}`, destructive: true, subject: source.id, object: target.id },
      ];
    default:
      return [];
  }
}
