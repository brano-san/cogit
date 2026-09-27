export interface ConfirmRequest {
  title: string;
  message: string;
  confirm: string;
  warning?: boolean;
  /** Label of an unchecked checkbox; its state comes back from `askWithOption`. */
  option?: string;
}

interface Pending extends ConfirmRequest {
  settle: (value: boolean, checked: boolean) => void;
}

/** A yes-or-no question in the app's own modal, awaited like `prompt.ask`. */
class ConfirmStore {
  open = $state.raw<Pending | null>(null);

  async ask(request: ConfirmRequest): Promise<boolean> {
    return (await this.askWithOption(request)).yes;
  }

  askWithOption(request: ConfirmRequest): Promise<{ yes: boolean; checked: boolean }> {
    this.open?.settle(false, false);
    return new Promise((resolve) => {
      this.open = {
        ...request,
        settle: (yes, checked) => {
          this.open = null;
          resolve({ yes, checked: yes && checked });
        },
      };
    });
  }

  answer(value: boolean, checked = false): void {
    this.open?.settle(value, checked);
  }
}

export const confirmation = new ConfirmStore();
