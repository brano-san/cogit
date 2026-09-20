import type { CogitError } from "$lib/ipc";

class ErrorStore {
  /** Non-modal: the dialog never blocks the rest of the window (doc/05-ui-layout.md). */
  current = $state<CogitError | null>(null);

  report(error: CogitError | null): void {
    if (error) this.current = error;
  }

  dismiss(): void {
    this.current = null;
  }
}

export const errors = new ErrorStore();
