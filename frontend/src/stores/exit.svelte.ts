import type { Operation, OperationChanged } from "$lib/ipc";
import {
  applyPending,
  confirmExitAfter,
  exitVariant,
  mustAskBeforeExit,
  seedPending,
  settleVerdict,
  type ExitAction,
  type ExitSource,
  type ExitVariant,
} from "$lib/exit";

/** The Exit dialog from the close request to the answer, including the wait behind
    Exit When Done (doc/12-risks.md, R-151, R-168). */
export class ExitFlow {
  prompt = $state.raw<{ source: ExitSource } | null>(null);
  /** Queued and running operations, kept live while the dialog is open. */
  pending = $state.raw<Map<number, Operation>>(new Map());
  waiting = $state(false);

  #asking = false;
  #resolve: ((go: boolean) => void) | null = null;
  #meanwhile: OperationChanged[] | null = null;
  #failed = false;
  #next: ExitSource | null = null;

  get variant(): ExitVariant {
    return exitVariant(this.pending.size);
  }

  /** The close about to be requested comes from the menu or its shortcut. */
  fromCommand(): void {
    this.#next = "command";
  }

  takeSource(): ExitSource {
    const source = this.#next ?? "window";
    this.#next = null;
    return source;
  }

  observe(event: OperationChanged): void {
    this.#meanwhile?.push(event);
    if (!this.prompt) return;
    this.pending = applyPending(this.pending, event);
    if (this.waiting && event.phase === "done" && event.success === false) this.#failed = true;
    this.#settle();
  }

  /** Resolves true when the app may go. A second request while one is open is refused. */
  async ask(
    source: ExitSource,
    confirmExit: boolean,
    snapshot: () => Promise<Operation[]>,
  ): Promise<boolean> {
    if (this.#asking) return false;
    this.#asking = true;
    this.#meanwhile = [];
    const operations = await Promise.resolve()
      .then(snapshot)
      .catch(() => []);
    const pending = seedPending(operations, this.#meanwhile);
    this.#meanwhile = null;

    if (!mustAskBeforeExit(confirmExit, pending.size, source)) {
      this.#asking = false;
      return true;
    }
    this.pending = pending;
    this.waiting = false;
    this.#failed = false;
    this.prompt = { source };
    return new Promise((resolve) => (this.#resolve = resolve));
  }

  /** Returns the `confirmExit` to store, or null to leave the setting alone. */
  answer(action: ExitAction, dontShowAgain: boolean, confirmExit: boolean): boolean | null {
    const stored = confirmExitAfter(action, this.variant, dontShowAgain, confirmExit);
    if (action === "exitWhenDone") {
      this.waiting = true;
      this.#failed = false;
      this.#settle();
      return null;
    }
    this.#finish(action !== "cancel");
    return stored;
  }

  #settle(): void {
    const source = this.prompt?.source ?? "window";
    const verdict = settleVerdict(source, this.waiting, this.pending.size, this.#failed);
    if (verdict !== "wait") this.#finish(verdict === "exit");
  }

  #finish(go: boolean): void {
    const resolve = this.#resolve;
    this.#resolve = null;
    this.#asking = false;
    this.prompt = null;
    this.waiting = false;
    resolve?.(go);
  }
}

export const exitFlow = new ExitFlow();
