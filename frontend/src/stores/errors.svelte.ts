import { CogitError } from "$lib/ipc";
import { output } from "$stores/output.svelte";

/** A failing operation can report itself once per panel that reads its store, and a loop
    can report for ever. The queue is short on purpose: the twentieth failure in a row
    says nothing the first one did not. */
const KEPT = 20;

class ErrorStore {
  /** Newest in front: the last thing that went wrong is the thing being looked at. */
  #queue = $state.raw<CogitError[]>([]);

  get current(): CogitError | null {
    return this.#queue[0] ?? null;
  }

  /** Drives the counter on the notification and the status in the footer (item 4). */
  get pending(): number {
    return this.#queue.length;
  }

  /** Anything carrying raw git output belongs in the output window, where it can be
      searched and read at length; this store keeps the refusals Cogit made on its own. */
  report(error: CogitError | null): void {
    if (!error) return;
    if (error.detail.kind === "command") {
      void output.raise(error.detail.data.id);
      return;
    }
    if (this.current?.message === error.message) return;
    this.#queue = [error, ...this.#queue].slice(0, KEPT);
  }

  /** For a refusal Cogit decided on itself, with no Git output behind it. */
  message(text: string): void {
    this.report(new CogitError({ kind: "invalidState", data: text }));
  }

  dismiss(): void {
    this.#queue = this.#queue.slice(1);
  }

  dismissAll(): void {
    this.#queue = [];
  }
}

export const errors = new ErrorStore();
