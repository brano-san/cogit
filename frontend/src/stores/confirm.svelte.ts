export interface ConfirmRequest {
  title: string;
  message: string;
  confirm: string;
  warning?: boolean;
}

interface Pending extends ConfirmRequest {
  settle: (value: boolean) => void;
}

/** A yes-or-no question in the app's own modal, awaited like `prompt.ask`. */
class ConfirmStore {
  open = $state.raw<Pending | null>(null);

  ask(request: ConfirmRequest): Promise<boolean> {
    this.open?.settle(false);
    return new Promise((resolve) => {
      this.open = {
        ...request,
        settle: (value) => {
          this.open = null;
          resolve(value);
        },
      };
    });
  }

  answer(value: boolean): void {
    this.open?.settle(value);
  }
}

export const confirmation = new ConfirmStore();
