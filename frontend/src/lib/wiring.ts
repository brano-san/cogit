import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import {
  onAvatarReady,
  onCommandRecorded,
  onMergeResolved,
  onOperationChanged,
  onRepoChanged,
  type CommandNotice,
  type MergeResolved,
  type OperationChanged,
  type RepoChanged,
} from "$lib/ipc";
import { counted } from "$lib/listener-count";

/** What the window listens to from outside itself. Everything is a `() => Promise<stop>`,
    which is the one shape every Tauri listener has, so they unsubscribe together. */
export interface Handlers {
  repoChanged: (event: RepoChanged) => void;
  operationChanged: (event: OperationChanged) => void;
  avatarReady: (email: string) => void;
  mergeResolved: (event: MergeResolved) => void;
  /** One per git command, whatever it did. */
  commandRecorded: (event: CommandNotice) => void;
  /** Return false to keep the window open. */
  closeRequested: () => Promise<boolean>;
  dragDrop: (event: DragDropEvent) => void;
}

type Stop = () => void;

/** Subscribes to all of them and gives back one function that undoes the lot. Errors are
    swallowed on purpose: a listener that never attached has nothing to detach.

    The `on*` helpers count themselves inside `$lib/ipc`; the two that come straight
    from the Tauri API are counted here, so the probe sees every subscription this window
    holds. */
export function connect(handlers: Handlers): Stop {
  const pending: Promise<Stop>[] = [
    onRepoChanged(handlers.repoChanged),
    onOperationChanged(handlers.operationChanged),
    onAvatarReady((event) => handlers.avatarReady(event.email)),
    onMergeResolved(handlers.mergeResolved),
    onCommandRecorded(handlers.commandRecorded),
    counted(
      getCurrentWindow().onCloseRequested(async (event) => {
        if (!(await handlers.closeRequested())) event.preventDefault();
      }),
    ),
    counted(
      getCurrentWebview().onDragDropEvent((event) => handlers.dragDrop(event.payload)),
    ),
  ];

  return () => {
    for (const listener of pending) {
      void listener.then((stop) => stop()).catch(() => {});
    }
  };
}
