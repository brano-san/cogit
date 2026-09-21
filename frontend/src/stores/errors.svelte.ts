import { CogitError } from "$lib/ipc";
import { output } from "$stores/output.svelte";

class ErrorStore {
  current = $state<CogitError | null>(null);

  /** Anything carrying raw git output belongs in the output window, where it can be
      searched and read at length; this store keeps the refusals Cogit made on its own. */
  report(error: CogitError | null): void {
    if (!error) return;
    if (error.detail.kind === "command") {
      void output.raise(error.detail.data.id);
      return;
    }
    this.current = error;
  }

  /** For a refusal Cogit decided on itself, with no Git output behind it. */
  message(text: string): void {
    this.current = new CogitError({ kind: "invalidState", data: text });
  }

  dismiss(): void {
    this.current = null;
  }
}

export const errors = new ErrorStore();
