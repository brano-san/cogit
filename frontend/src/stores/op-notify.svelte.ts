import { showNotification, type Operation, type OperationKind, type RepoId } from "$lib/ipc";
import { notificationFor, type Outcome } from "$lib/op-notify";
import { errorWindow } from "$stores/error-window.svelte";
import { settings } from "$stores/settings.svelte";
import { successToast } from "$stores/success-toast.svelte";

/** The toast and the Errors window entry land just after the operation's `done`. */
const SETTLE_MS = 400;
/** Long enough to be worth a bar on the taskbar button; staging, tagging and the like are not. */
const BAR_KINDS = new Set<OperationKind>(["fetch", "pull", "push", "clone", "merge", "rebase", "submodule", "other"]);

/** Times every queued operation, so the one that ends in the background after a while can
    tell the system; and keeps the count of long ones running for the taskbar bar. */
class OperationNotices {
  #started = new Map<number, number>();
  #barred = new Set<number>();
  busy = $state(0);

  /** `focused` is asked when the result is in, not when the operation ended. */
  observe(op: Operation, nameOf: (repo: RepoId) => string | null, focused: () => boolean): void {
    if (op.phase === "running") {
      if (!this.#started.has(op.id)) this.#started.set(op.id, performance.now());
      if (BAR_KINDS.has(op.kind) && !this.#barred.has(op.id)) {
        this.#barred.add(op.id);
        this.busy = this.#barred.size;
      }
      return;
    }
    if (op.phase !== "done") return;
    if (this.#barred.delete(op.id)) this.busy = this.#barred.size;
    const started = this.#started.get(op.id);
    this.#started.delete(op.id);
    if (started === undefined) return;
    const ms = performance.now() - started;
    const toast = successToast.id;
    const known = new Set(errorWindow.entries.map((entry) => entry.id));
    setTimeout(() => {
      const fresh = errorWindow.entries.filter((entry) => !known.has(entry.id));
      const outcome: Outcome =
        op.success === false || fresh.some((entry) => entry.kind === "error")
          ? "failure"
          : fresh.length > 0
            ? "attention"
            : "success";
      const shown = notificationFor(
        {
          kind: op.kind,
          label: op.label,
          repoName: op.repo === null ? null : nameOf(op.repo),
          outcome,
          ms,
          summary: successToast.id !== toast ? successToast.text : null,
        },
        {
          enabled: settings.current.notificationsSystem,
          success: settings.current.notificationsSystemSuccess,
          failure: settings.current.notificationsSystemFailure,
        },
        focused(),
      );
      if (shown) {
        void showNotification(shown.title, shown.body, op.repo, shown.failed).catch(() => {
          // No system to tell (a browser, a headless run): the in-app result stands.
        });
      }
    }, SETTLE_MS);
  }
}

export const operationNotices = new OperationNotices();
