export interface ConfirmRequest {
  title: string;
  message: string;
  items?: readonly string[];
  confirm: string;
  danger?: boolean;
}

interface Pending extends ConfirmRequest {
  settle: (yes: boolean) => void;
}

/** A yes/no question in the app's own modal: the file and repository menus must not use
    the browser's `confirm` or a system message box (#36, #40). */
class ConfirmDialogStore {
  open = $state.raw<Pending | null>(null);

  ask(request: ConfirmRequest): Promise<boolean> {
    this.open?.settle(false);
    return new Promise((resolve) => {
      this.open = {
        ...request,
        settle: (yes) => {
          this.open = null;
          resolve(yes);
        },
      };
    });
  }

  answer(yes: boolean): void {
    this.open?.settle(yes);
  }
}

export const confirmDialog = new ConfirmDialogStore();
