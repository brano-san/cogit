import { shortOid } from "$lib/format";
import { clickedCommit } from "$lib/graph-geometry";
export type DragKind = "branch" | "commit";

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
): string | null {
  if (y < 0 || rowHeight <= 0) return null;
  const row = Math.floor((y + scrollTop) / rowHeight);
  if (row >= listRows) return null;
  return clickedCommit(row, headerRows, oidAt) ?? null;
}

export interface DragPayload {
  kind: DragKind;
  id: string;
}

export type DropActionId = "merge" | "rebase" | "fastForward" | "squash" | "reorder";

export interface DropAction {
  id: DropActionId;
  title: string;
  /** Rewrites history, so the caller confirms before running it. */
  destructive: boolean;
  /** Why it cannot run from here; shown on the item, which stays in the menu. */
  disabled?: string;
}

export const DRAG_TYPE = "application/x-cogit";

export function serialiseDrag(payload: DragPayload): string {
  return JSON.stringify(payload);
}

/** The browser will hand over whatever was dragged, including text from another app. */
export function parseDrag(text: string): DragPayload | null {
  try {
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== "object" || parsed === null) return null;
    const { kind, id } = parsed as { kind?: unknown; id?: unknown };
    if ((kind !== "branch" && kind !== "commit") || typeof id !== "string" || id === "") {
      return null;
    }
    return { kind, id };
  } catch {
    return null;
  }
}

/** `head` is the checked-out branch: git merges into it and rebases it, and into or of
    no other, so a merge into another branch or a rebase of one is offered disabled. */
export function dropActions(
  source: DragPayload,
  target: DragPayload,
  canFastForward: boolean,
  head: string | null = null,
): DropAction[] {
  if (source.kind !== target.kind || source.id === target.id) return [];

  if (source.kind === "branch") {
    const merge: DropAction = { id: "merge", title: `Merge ${source.id} into ${target.id}`, destructive: false };
    if (target.id !== head) merge.disabled = `Check out ${target.id} first: a merge goes into the checked-out branch.`;
    const rebase: DropAction = { id: "rebase", title: `Rebase ${source.id} onto ${target.id}`, destructive: true };
    if (source.id !== head) rebase.disabled = `Check out ${source.id} first: a rebase moves the checked-out branch.`;
    const actions: DropAction[] = [merge, rebase];
    if (canFastForward) {
      actions.push({
        id: "fastForward",
        title: `Fast-forward ${target.id} to ${source.id}`,
        destructive: false,
      });
    }
    return actions;
  }

  return [
    {
      id: "squash",
      title: `Squash ${shortOid(source.id)} into ${shortOid(target.id)}`,
      destructive: true,
    },
    {
      id: "reorder",
      title: `Move ${shortOid(source.id)} next to ${shortOid(target.id)}`,
      destructive: true,
    },
  ];
}
