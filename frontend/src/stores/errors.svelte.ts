import { CogitError } from "$lib/ipc";

class ErrorStore {
  current = $state<CogitError | null>(null);

  report(error: CogitError | null): void {
    if (error) this.current = error;
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
