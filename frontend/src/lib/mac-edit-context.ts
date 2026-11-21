import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { modals } from "./modal-stack";
import { ON_MAC } from "./platform";
import { isEditable, OWN_SELECTION } from "./select-all";

export type EditSurface = "files" | "solver" | null;

type EditFocus = "field" | "editor" | "output" | "other";

export function editRoles(surface: EditSurface, focus: EditFocus, modal: boolean) {
  const editable = focus === "field" || focus === "editor";
  const ownsUndo = focus === "other" || (surface === "solver" && focus === "editor");
  return { nativeSelectAll: editable, nativeUndo: modal || surface === null || !ownsUndo };
}

export function editContext(surface: EditSurface, target: Element | null, modal: boolean) {
  let focus: EditFocus = "other";
  if (target?.closest(".cm-editor")) focus = "editor";
  else if (isEditable(target)) focus = "field";
  else if (target?.closest(`[${OWN_SELECTION}]`)) focus = "output";
  return editRoles(surface, focus, modal);
}

export function installMacEditContext(win: Window, surface: EditSurface): () => void {
  if (!ON_MAC) return () => {};
  const label = getCurrentWindow().label;
  let last: ReturnType<typeof editContext> | undefined;
  let stopped = false;
  function publish() {
    if (stopped) return;
    const next = editContext(surface, win.document.activeElement, modals.any);
    if (last?.nativeSelectAll === next.nativeSelectAll && last.nativeUndo === next.nativeUndo) return;
    last = next;
    void emit("mac-edit-context", { label, ...next }).catch((error) => {
      last = undefined;
      console.error("Could not update the macOS Edit menu", error);
    });
  }
  const schedule = () => queueMicrotask(publish);
  publish();
  win.addEventListener("focusin", schedule);
  win.addEventListener("focusout", schedule);
  win.addEventListener("focus", schedule);
  return () => {
    stopped = true;
    win.removeEventListener("focusin", schedule);
    win.removeEventListener("focusout", schedule);
    win.removeEventListener("focus", schedule);
  };
}
