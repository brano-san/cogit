export interface ConfirmRequest {
  title: string;
  message: string;
  confirm: string;
  warning?: boolean;
  /** Label and value pairs under the message, the value in monospace: what is acted on. */
  fields?: readonly (readonly [string, string])[];
  /** A caution of its own under the rest, with a warning mark, in the muted color. */
  note?: string;
  /** Listed under the message in a scrollable box, with no cap: what is about to be deleted. */
  items?: readonly string[];
  /** Label of an unchecked checkbox; its state comes back from `askWithOption`. */
  option?: string;
  /** A dropdown under the message; the option picked comes back from `askWithChoice`. */
  choice?: ConfirmChoice;
}

export interface ConfirmChoice {
  label: string;
  value: string;
  options: readonly (readonly [string, string])[];
}

interface Pending extends ConfirmRequest {
  settle: (value: boolean, checked: boolean, choice?: string) => void;
}

/** A yes-or-no question in the app's own modal, awaited like `prompt.ask`. */
class ConfirmStore {
  open = $state.raw<Pending | null>(null);

  async ask(request: ConfirmRequest): Promise<boolean> {
    return (await this.askWithOption(request)).yes;
  }

  askWithOption(request: ConfirmRequest): Promise<{ yes: boolean; checked: boolean }> {
    return this.#ask(request).then(({ yes, checked }) => ({ yes, checked }));
  }

  askWithChoice(request: ConfirmRequest): Promise<{ yes: boolean; choice: string | null }> {
    return this.#ask(request).then(({ yes, choice }) => ({ yes, choice }));
  }

  #ask(request: ConfirmRequest): Promise<{ yes: boolean; checked: boolean; choice: string | null }> {
    this.open?.settle(false, false);
    return new Promise((resolve) => {
      this.open = {
        ...request,
        settle: (yes, checked, choice) => {
          this.open = null;
          resolve({ yes, checked: yes && checked, choice: yes ? (choice ?? request.choice?.value ?? null) : null });
        },
      };
    });
  }

  answer(value: boolean, checked = false, choice?: string): void {
    this.open?.settle(value, checked, choice);
  }
}

export const confirmation = new ConfirmStore();
