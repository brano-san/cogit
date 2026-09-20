import type { CogitError } from "$lib/ipc";

class ErrorStore {
  current = $state<CogitError | null>(null);

  report(error: CogitError | null): void {
    if (error) this.current = error;
  }

  dismiss(): void {
    this.current = null;
  }
}

export const errors = new ErrorStore();
