import { shortOid } from "$lib/format";
export type DragKind = "branch" | "commit";

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

export function dropActions(
  source: DragPayload,
  target: DragPayload,
  canFastForward: boolean,
): DropAction[] {
  if (source.kind !== target.kind || source.id === target.id) return [];

  if (source.kind === "branch") {
    const actions: DropAction[] = [
      { id: "merge", title: `Merge ${source.id} into ${target.id}`, destructive: false },
      { id: "rebase", title: `Rebase ${source.id} onto ${target.id}`, destructive: true },
    ];
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
